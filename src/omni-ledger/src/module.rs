use async_trait::async_trait;
use minicbor::decode;
use omni::message::{RequestMessage, ResponseMessage};
use omni::protocol::Attribute;
use omni::server::module::OmniModuleInfo;
use omni::{Identity, OmniError, OmniModule};
use omni_abci::module::OmniAbciModuleBackend;
use omni_abci::types::{AbciBlock, AbciCommitInfo, AbciInfo, AbciInit, EndpointInfo};
use std::cmp::max;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};
use tracing::{debug, info};

pub mod account;
pub mod ledger;

use crate::module::ledger::list::{Timestamp, Transaction, TransactionContent, TransactionKind};
use crate::{error, storage::LedgerStorage};
use account::balance::BalanceReturns;
use account::balance::{BalanceArgs, SymbolList};
use account::burn::BurnArgs;
use account::info::InfoReturns;
use account::mint::MintArgs;
use account::send::SendArgs;
use error::unauthorized;

const MAXIMUM_TRANSACTION_COUNT: usize = 100;

type TxResult = Result<Transaction, OmniError>;
fn filter_account<'a>(
    it: Box<dyn Iterator<Item = TxResult> + 'a>,
    account: Option<Identity>,
) -> Box<dyn Iterator<Item = TxResult> + 'a> {
    let new_it: Box<dyn Iterator<Item = TxResult>> = if let Some(id) = account {
        Box::new(it.filter(move |t| match t {
            // Propagate the errors.
            Err(_) => true,
            Ok(Transaction {
                content: TransactionContent::Send { from, to, .. },
                ..
            }) => from == &id || to == &id,
            Ok(Transaction {
                content: TransactionContent::Mint { account, .. },
                ..
            }) => account == &id,
            Ok(Transaction {
                content: TransactionContent::Burn { account, .. },
                ..
            }) => account == &id,
        }))
    } else {
        it
    };

    new_it
}

fn filter_transaction_kind<'a>(
    it: Box<dyn Iterator<Item = TxResult> + 'a>,
    transaction_kind: Option<TransactionKind>,
) -> Box<dyn Iterator<Item = TxResult> + 'a> {
    match transaction_kind {
        Some(TransactionKind::Send) => Box::new(it.filter(|t| match t {
            Err(_) => true,
            Ok(Transaction {
                content: TransactionContent::Send { .. },
                ..
            }) => true,
            _ => false,
        })),
        Some(TransactionKind::Mint) => Box::new(it.filter(|t| match t {
            Err(_) => true,
            Ok(Transaction {
                content: TransactionContent::Mint { .. },
                ..
            }) => true,
            _ => false,
        })),
        Some(TransactionKind::Burn) => Box::new(it.filter(|t| match t {
            Err(_) => true,
            Ok(Transaction {
                content: TransactionContent::Burn { .. },
                ..
            }) => true,
            _ => false,
        })),
        _ => it,
    }
}

fn filter_symbol<'a>(
    it: Box<dyn Iterator<Item = TxResult> + 'a>,
    symbol: Option<String>,
) -> Box<dyn Iterator<Item = TxResult> + 'a> {
    let new_it: Box<dyn Iterator<Item = TxResult>> = if let Some(s) = symbol {
        Box::new(it.filter(move |t| match t {
            // Propagate the errors.
            Err(_) => true,
            Ok(Transaction {
                content: TransactionContent::Send { symbol, .. },
                ..
            }) => symbol == &s,
            Ok(Transaction {
                content: TransactionContent::Mint { symbol, .. },
                ..
            }) => symbol == &s,
            Ok(Transaction {
                content: TransactionContent::Burn { symbol, .. },
                ..
            }) => symbol == &s,
        }))
    } else {
        it
    };

    new_it
}

fn filter_date<'a>(
    it: Box<dyn Iterator<Item = TxResult> + 'a>,
    start: Option<Timestamp>,
    end: Option<Timestamp>,
) -> Box<dyn Iterator<Item = TxResult> + 'a> {
    let new_it: Box<dyn Iterator<Item = TxResult>> = if let Some(start) = start {
        Box::new(it.filter(move |t| match t {
            // Propagate the errors.
            Err(_) => true,
            Ok(Transaction { time, .. }) => time >= &start,
        }))
    } else {
        it
    };
    let new_it: Box<dyn Iterator<Item = TxResult>> = if let Some(end) = end {
        Box::new(new_it.take_while(move |t| match t {
            // Propagate the errors.
            Err(_) => true,
            Ok(Transaction { time, .. }) => time <= &end,
        }))
    } else {
        new_it
    };

    new_it
}

/// The initial state schema, loaded from JSON.
#[derive(serde::Deserialize, Debug, Default)]
pub struct InitialStateJson {
    initial: BTreeMap<Identity, BTreeMap<String, u128>>,
    symbols: BTreeSet<String>,
    minters: Option<BTreeMap<String, Vec<Identity>>>,
    hash: Option<String>,
}

/// A simple ledger that keeps transactions in memory.
#[derive(Debug, Clone)]
pub struct LedgerModule {
    storage: Arc<Mutex<LedgerStorage>>,
}

impl LedgerModule {
    pub fn new<P: AsRef<Path>>(
        initial_state: Option<InitialStateJson>,
        persistence_store_path: P,
        blockchain: bool,
    ) -> Result<Self, OmniError> {
        let storage = if let Some(state) = initial_state {
            let storage = LedgerStorage::new(
                state.symbols,
                state.initial,
                state.minters.unwrap_or_default(),
                persistence_store_path,
                blockchain,
            )
            .map_err(OmniError::unknown)?;

            if let Some(h) = state.hash {
                // Verify the hash.
                let actual = hex::encode(storage.hash());
                if actual != h {
                    return Err(error::invalid_initial_state(h, actual));
                }
            }

            storage
        } else {
            LedgerStorage::load(persistence_store_path, blockchain).unwrap()
        };

        info!(
            height = storage.get_height(),
            hash = hex::encode(storage.hash()).as_str()
        );

        Ok(Self {
            storage: Arc::new(Mutex::new(storage)),
        })
    }

    fn account_info(&self, _payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let storage = self.storage.lock().unwrap();

        // Hash the storage.
        let hash = storage.hash();
        let symbols: Vec<&str> = storage.get_symbols();

        info!(
            "info(): hash={} symbols={:?}",
            hex::encode(storage.hash()).as_str(),
            symbols
        );

        minicbor::to_vec(InfoReturns {
            symbols: symbols.iter().map(|x| x.to_string()).collect(),
            hash: hash.into(),
        })
        .map_err(|e| OmniError::serialization_error(e.to_string()))
    }

    fn balance(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let BalanceArgs {
            account,
            symbols,
            proof,
        } = decode(payload).map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let identity = account.as_ref().unwrap_or(from);

        let mut storage = self.storage.lock().unwrap();
        let symbols = symbols.unwrap_or_else(|| SymbolList(BTreeSet::new())).0;

        let balances = storage.get_multiple_balances(identity, &symbols);
        info!("balance({}, {:?}): {:?}", identity, &symbols, &balances);
        let returns = if proof.unwrap_or(false) {
            BalanceReturns {
                balances: None,
                proof: Some(storage.generate_proof(identity, &symbols)?.into()),
                hash: storage.hash().into(),
            }
        } else {
            BalanceReturns {
                balances: Some(
                    balances
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), v))
                        .collect(),
                ),
                proof: None,
                hash: storage.hash().into(),
            }
        };

        minicbor::to_vec(returns).map_err(|e| OmniError::serialization_error(e.to_string()))
    }

    fn send(&self, sender: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let SendArgs {
            from,
            to,
            amount,
            symbol,
        } = decode(payload).map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let from = from.as_ref().unwrap_or(sender);

        // TODO: allow some ACLs or delegation on the ledger.
        if from != sender {
            return Err(unauthorized());
        }

        let mut storage = self.storage.lock().unwrap();
        storage.send(from, &to, &symbol.to_string(), amount)?;
        minicbor::to_vec(()).map_err(|e| OmniError::serialization_error(e.to_string()))
    }

    fn mint(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let MintArgs {
            account,
            amount,
            symbol,
        } = decode(payload).map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let mut storage = self.storage.lock().unwrap();

        if storage.can_mint(from, symbol) {
            storage.mint(&account, &symbol.to_string(), amount)?;
        } else {
            return Err(unauthorized());
        }

        minicbor::to_vec(()).map_err(|e| OmniError::serialization_error(e.to_string()))
    }

    fn burn(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let BurnArgs {
            account,
            amount,
            symbol,
        } = decode(payload).map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let mut storage = self.storage.lock().unwrap();
        if storage.can_mint(from, symbol) {
            storage.burn(&account, &symbol.to_string(), amount)?;
        } else {
            return Err(unauthorized());
        }

        minicbor::to_vec(()).map_err(|e| OmniError::serialization_error(e.to_string()))
    }

    fn ledger_info(&self, _sender: &Identity, _payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let storage = self.storage.lock().unwrap();
        minicbor::to_vec(ledger::info::InfoReturns {
            nb_transactions: storage.nb_transactions(),
        })
        .map_err(|e| OmniError::serialization_error(e.to_string()))
    }

    fn ledger_list(&self, _sender: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let ledger::list::ListArgs {
            count,
            account,
            min_id,
            transaction_type,
            date_start,
            date_end,
            symbol,
        } = decode(payload).map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let count = count.map_or(MAXIMUM_TRANSACTION_COUNT, |c| {
            max(c as usize, MAXIMUM_TRANSACTION_COUNT)
        });

        let storage = self.storage.lock().unwrap();
        let nb_transactions = storage.nb_transactions();
        let iter = storage.iter(min_id);

        let iter = Box::new(iter.take(count).map(|(_k, v)| {
            decode::<Transaction>(v.as_slice())
                .map_err(|e| OmniError::deserialization_error(e.to_string()))
        }));

        let iter = filter_account(iter, account);
        let iter = filter_transaction_kind(iter, transaction_type);
        let iter = filter_symbol(iter, symbol);
        let iter = filter_date(iter, date_start, date_end);

        let transactions: Vec<Transaction> = iter
            .collect::<Result<_, _>>()
            .map_err(|e| OmniError::unknown(e.to_string()))?;

        minicbor::to_vec(ledger::list::ListReturns {
            nb_transactions,
            transactions,
        })
        .map_err(|e| OmniError::serialization_error(e.to_string()))
    }
}

// This module is always supported, but will only be added when created using an ABCI
// flag.
impl OmniAbciModuleBackend for LedgerModule {
    #[rustfmt::skip]
    fn init(&self) -> AbciInit {
        AbciInit {
            endpoints: BTreeMap::from([
                ("account.info".to_string(), EndpointInfo { should_commit: false }),
                ("account.balance".to_string(), EndpointInfo { should_commit: false }),
                ("account.mint".to_string(), EndpointInfo { should_commit: true }),
                ("account.burn".to_string(), EndpointInfo { should_commit: true }),
                ("account.send".to_string(), EndpointInfo { should_commit: true }),
                ("ledger.info".to_string(), EndpointInfo { should_commit: false }),
                ("ledger.list".to_string(), EndpointInfo { should_commit: false }),
            ]),
        }
    }

    fn init_chain(&self) -> Result<(), OmniError> {
        info!("abci.init_chain()",);
        Ok(())
    }

    fn block_begin(&self, info: AbciBlock) -> Result<(), OmniError> {
        let time = info.time;
        info!("abci.block_begin(): time={:?}", time);

        if let Some(time) = time {
            let time = UNIX_EPOCH.checked_add(Duration::from_secs(time)).unwrap();
            let mut storage = self.storage.lock().unwrap();
            storage.set_time(time);
        }

        Ok(())
    }

    fn info(&self) -> Result<AbciInfo, OmniError> {
        let storage = self.storage.lock().unwrap();

        info!(
            "abci.info(): height={} hash={}",
            storage.get_height(),
            hex::encode(storage.hash()).as_str()
        );
        Ok(AbciInfo {
            height: storage.get_height(),
            hash: storage.hash().into(),
        })
    }

    fn commit(&self) -> Result<AbciCommitInfo, OmniError> {
        let mut storage = self.storage.lock().unwrap();
        let result = storage.commit();

        info!(
            "abci.commit(): retain_height={} hash={}",
            result.retain_height,
            hex::encode(storage.hash()).as_str()
        );
        Ok(result)
    }
}

pub const ACCOUNT_ATTRIBUTE: Attribute = Attribute::id(2);
pub const LEDGER_ATTRIBUTE: Attribute = Attribute::id(4);

lazy_static::lazy_static!(
    pub static ref LEDGER_MODULE_INFO: OmniModuleInfo = OmniModuleInfo {
        name: "LedgerModule".to_string(),
        attributes: vec![ACCOUNT_ATTRIBUTE, LEDGER_ATTRIBUTE],
        endpoints: vec![
            "account.info".to_string(),
            "account.balance".to_string(),
            "account.mint".to_string(),
            "account.burn".to_string(),
            "account.send".to_string(),
            "ledger.info".to_string(),
            "ledger.list".to_string(),
        ]
    };
);

#[async_trait]
impl OmniModule for LedgerModule {
    fn info(&self) -> &OmniModuleInfo {
        &LEDGER_MODULE_INFO
    }

    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        match message.method.as_str() {
            "account.info" => return Ok(()),
            "account.mint" => {
                decode::<'_, MintArgs>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
            }
            "account.burn" => {
                decode::<'_, BurnArgs>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
            }
            "account.balance" => {
                decode::<'_, BalanceArgs>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
            }
            "account.send" => {
                decode::<'_, SendArgs>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
            }
            "ledger.info" => return Ok(()),
            "ledger.list" => {
                decode::<'_, ledger::list::ListArgs>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
            }
            _ => {
                return Err(OmniError::internal_server_error());
            }
        };
        Ok(())
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let data = match message.method.as_str() {
            "account.info" => self.account_info(&message.data),
            "account.balance" => self.balance(&message.from.unwrap_or_default(), &message.data),
            "account.mint" => self.mint(&message.from.unwrap_or_default(), &message.data),
            "account.burn" => self.burn(&message.from.unwrap_or_default(), &message.data),
            "account.send" => self.send(&message.from.unwrap_or_default(), &message.data),
            "ledger.info" => self.ledger_info(&message.from.unwrap_or_default(), &message.data),
            "ledger.list" => self.ledger_list(&message.from.unwrap_or_default(), &message.data),
            _ => Err(OmniError::internal_server_error()),
        }?;

        Ok(ResponseMessage::from_request(
            &message,
            &message.to,
            Ok(data),
        ))
    }
}
