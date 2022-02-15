use crate::{error, storage::LedgerStorage};
use minicbor::decode;
use omni::server::module::abci_backend::{
    AbciBlock, AbciCommitInfo, AbciInfo, AbciInit, EndpointInfo, OmniAbciModuleBackend,
};
use omni::server::module::{ledger, ledger_transactions};
use omni::types::ledger::{Symbol, TokenAmount, Transaction, TransactionKind};
use omni::types::{CborRange, Timestamp, VecOrSingle};
use omni::{Identity, OmniError};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};
use tracing::info;

const MAXIMUM_TRANSACTION_COUNT: usize = 100;

type TxResult = Result<Transaction, OmniError>;
fn filter_account<'a>(
    it: Box<dyn Iterator<Item = TxResult> + 'a>,
    account: Option<VecOrSingle<Identity>>,
) -> Box<dyn Iterator<Item = TxResult> + 'a> {
    if let Some(account) = account {
        let account: Vec<Identity> = account.into();
        Box::new(it.filter(move |t| match t {
            // Propagate the errors.
            Err(_) => true,
            Ok(t) => account.iter().any(|id| t.is_about(id)),
        }))
    } else {
        it
    }
}

fn filter_transaction_kind<'a>(
    it: Box<dyn Iterator<Item = TxResult> + 'a>,
    transaction_kind: Option<VecOrSingle<TransactionKind>>,
) -> Box<dyn Iterator<Item = TxResult> + 'a> {
    if let Some(k) = transaction_kind {
        let k: Vec<TransactionKind> = k.into();
        Box::new(it.filter(move |t| match t {
            Err(_) => true,
            Ok(t) => k.contains(&t.kind()),
        }))
    } else {
        it
    }
}

fn filter_symbol<'a>(
    it: Box<dyn Iterator<Item = TxResult> + 'a>,
    symbol: Option<VecOrSingle<String>>,
) -> Box<dyn Iterator<Item = TxResult> + 'a> {
    if let Some(s) = symbol {
        let s: Vec<String> = s.into();
        Box::new(it.filter(move |t| match t {
            // Propagate the errors.
            Err(_) => true,
            Ok(t) => s.contains(t.symbol()),
        }))
    } else {
        it
    }
}

fn filter_date<'a>(
    it: Box<dyn Iterator<Item = TxResult> + 'a>,
    range: CborRange<Timestamp>,
) -> Box<dyn Iterator<Item = TxResult> + 'a> {
    Box::new(it.filter(move |t| match t {
        // Propagate the errors.
        Err(_) => true,
        Ok(Transaction { time, .. }) => range.contains(time),
    }))
}

/// The initial state schema, loaded from JSON.
#[derive(serde::Deserialize, Debug, Default)]
pub struct InitialStateJson {
    initial: BTreeMap<Identity, BTreeMap<Symbol, TokenAmount>>,
    symbols: BTreeMap<Identity, String>,
    minters: Option<BTreeMap<Symbol, Vec<Identity>>>,
    hash: Option<String>,
}

/// A simple ledger that keeps transactions in memory.
#[derive(Debug)]
pub struct LedgerModuleImpl {
    storage: LedgerStorage,
}

impl LedgerModuleImpl {
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

        Ok(Self { storage })
    }
}

impl ledger::LedgerModuleBackend for LedgerModuleImpl {
    fn info(
        &self,
        _sender: &Identity,
        _args: ledger::InfoArgs,
    ) -> Result<ledger::InfoReturns, OmniError> {
        let storage = &self.storage;

        // Hash the storage.
        let hash = storage.hash();
        let symbols = storage.get_symbols();

        info!(
            "info(): hash={} symbols={:?}",
            hex::encode(storage.hash()).as_str(),
            symbols
        );

        Ok(ledger::InfoReturns {
            symbols: symbols.keys().map(|x| x.clone()).collect(),
            hash: hash.into(),
            local_names: symbols.clone(),
        })
    }

    fn balance(
        &self,
        sender: &Identity,
        args: ledger::BalanceArgs,
    ) -> Result<ledger::BalanceReturns, OmniError> {
        let ledger::BalanceArgs { account, symbols } = args;

        let identity = account.as_ref().unwrap_or(sender);

        let storage = &self.storage;
        let symbols = symbols.unwrap_or_default().0;

        let balances = storage
            .get_multiple_balances(identity, &BTreeSet::from_iter(symbols.clone().into_iter()));
        info!("balance({}, {:?}): {:?}", identity, &symbols, &balances);
        Ok(ledger::BalanceReturns {
            balances: balances.into_iter().map(|(k, v)| (k.clone(), v)).collect(),
        })
    }

    fn mint(&mut self, sender: &Identity, args: ledger::MintArgs) -> Result<(), OmniError> {
        let ledger::MintArgs {
            account,
            amount,
            symbol,
        } = args;

        let storage = &mut self.storage;
        if storage.can_mint(sender, &symbol) {
            storage.mint(&account, &symbol, amount)?;
        } else {
            return Err(error::unauthorized());
        }

        Ok(())
    }

    fn burn(&mut self, sender: &Identity, args: ledger::BurnArgs) -> Result<(), OmniError> {
        let ledger::BurnArgs {
            account,
            amount,
            symbol,
        } = args;

        if self.storage.can_mint(sender, &symbol) {
            self.storage.burn(&account, &symbol, amount)?;
        } else {
            return Err(error::unauthorized());
        }

        Ok(())
    }

    fn send(&mut self, sender: &Identity, args: ledger::SendArgs) -> Result<(), OmniError> {
        let ledger::SendArgs {
            from,
            to,
            amount,
            symbol,
        } = args;

        let from = from.as_ref().unwrap_or(sender);

        // TODO: allow some ACLs or delegation on the ledger.
        if from != sender {
            return Err(error::unauthorized());
        }

        self.storage.send(from, &to, &symbol, amount)?;
        Ok(())
    }
}

impl ledger_transactions::LedgerTransactionsModuleBackend for LedgerModuleImpl {
    fn transactions(
        &self,
        _args: ledger_transactions::TransactionsArgs,
    ) -> Result<ledger_transactions::TransactionsReturns, OmniError> {
        Ok(ledger_transactions::TransactionsReturns {
            nb_transactions: self.storage.nb_transactions(),
        })
    }

    fn list(
        &mut self,
        args: ledger_transactions::ListArgs,
    ) -> Result<ledger_transactions::ListReturns, OmniError> {
        let ledger_transactions::ListArgs {
            count,
            order,
            filter,
        } = args;
        let filter = filter.unwrap_or_default();

        let count = count.map_or(MAXIMUM_TRANSACTION_COUNT, |c| {
            std::cmp::min(c as usize, MAXIMUM_TRANSACTION_COUNT)
        });

        let storage = &self.storage;
        let nb_transactions = storage.nb_transactions();
        let iter = storage.iter(
            filter.id_range.unwrap_or_default(),
            order.unwrap_or_default(),
        );

        let iter = Box::new(iter.map(|(_k, v)| {
            decode::<Transaction>(v.as_slice())
                .map_err(|e| OmniError::deserialization_error(e.to_string()))
        }));

        let iter = filter_account(iter, filter.account);
        let iter = filter_transaction_kind(iter, filter.kind);
        let iter = filter_symbol(iter, filter.symbol);
        let iter = filter_date(iter, filter.date_range.unwrap_or_default());

        let transactions: Vec<Transaction> = iter.take(count).collect::<Result<_, _>>()?;

        Ok(ledger_transactions::ListReturns {
            nb_transactions,
            transactions,
        })
    }
}

// This module is always supported, but will only be added when created using an ABCI
// flag.
impl OmniAbciModuleBackend for LedgerModuleImpl {
    #[rustfmt::skip]
    fn init(&mut self) -> Result<AbciInit, OmniError> {
        Ok(AbciInit {
            endpoints: BTreeMap::from([
                ("ledger.info".to_string(), EndpointInfo { is_command: false }),
                ("ledger.balance".to_string(), EndpointInfo { is_command: false }),
                ("ledger.mint".to_string(), EndpointInfo { is_command: true }),
                ("ledger.burn".to_string(), EndpointInfo { is_command: true }),
                ("ledger.send".to_string(), EndpointInfo { is_command: true }),
                ("ledger.transactions".to_string(), EndpointInfo { is_command: false }),
                ("ledger.list".to_string(), EndpointInfo { is_command: false }),
            ]),
        })
    }

    fn init_chain(&mut self) -> Result<(), OmniError> {
        info!("abci.init_chain()",);
        Ok(())
    }

    fn block_begin(&mut self, info: AbciBlock) -> Result<(), OmniError> {
        let time = info.time;
        info!("abci.block_begin(): time={:?}", time);

        if let Some(time) = time {
            let time = UNIX_EPOCH.checked_add(Duration::from_secs(time)).unwrap();
            self.storage.set_time(time);
        }

        Ok(())
    }

    fn info(&self) -> Result<AbciInfo, OmniError> {
        let storage = &self.storage;

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

    fn commit(&mut self) -> Result<AbciCommitInfo, OmniError> {
        let result = self.storage.commit();

        info!(
            "abci.commit(): retain_height={} hash={}",
            result.retain_height,
            hex::encode(result.hash.as_slice()).as_str()
        );
        Ok(result)
    }
}
