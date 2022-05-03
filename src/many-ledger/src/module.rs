use crate::{error, storage::LedgerStorage};
use many::server::module::abci_backend::{
    AbciBlock, AbciCommitInfo, AbciInfo, AbciInit, AbciListSnapshot, EndpointInfo,
    ManyAbciModuleBackend,
};
use many::server::module::ledger;
use many::types::ledger::{Symbol, TokenAmount, Transaction, TransactionKind};
use many::types::{CborRange, Timestamp, VecOrSingle};
use many::{Identity, ManyError};
use minicbor::decode;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};
use tracing::info;

const MAXIMUM_TRANSACTION_COUNT: usize = 100;

type TxResult = Result<Transaction, ManyError>;
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
        snapshot_path: P,
        blockchain: bool,
    ) -> Result<Self, ManyError> {
        let storage = if let Some(state) = initial_state {
            let storage = LedgerStorage::new(
                state.symbols,
                state.initial,
                persistence_store_path,
                snapshot_path,
                blockchain,
            )
            .map_err(ManyError::unknown)?;

            if let Some(h) = state.hash {
                // Verify the hash.
                let actual = hex::encode(storage.hash());
                if actual != h {
                    return Err(error::invalid_initial_state(h, actual));
                }
            }

            storage
        } else {
            LedgerStorage::load(persistence_store_path, snapshot_path, blockchain).unwrap()
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
    ) -> Result<ledger::InfoReturns, ManyError> {
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
            symbols: symbols.keys().copied().collect(),
            hash: hash.into(),
            local_names: symbols,
        })
    }

    fn balance(
        &self,
        sender: &Identity,
        args: ledger::BalanceArgs,
    ) -> Result<ledger::BalanceReturns, ManyError> {
        let ledger::BalanceArgs { account, symbols } = args;

        let identity = account.as_ref().unwrap_or(sender);

        let storage = &self.storage;
        let symbols = symbols.unwrap_or_default().0;

        let balances = storage
            .get_multiple_balances(identity, &BTreeSet::from_iter(symbols.clone().into_iter()));
        info!("balance({}, {:?}): {:?}", identity, &symbols, &balances);
        Ok(ledger::BalanceReturns {
            balances: balances.into_iter().map(|(k, v)| (*k, v)).collect(),
        })
    }
}

impl ledger::LedgerCommandsModuleBackend for LedgerModuleImpl {
    fn send(&mut self, sender: &Identity, args: ledger::SendArgs) -> Result<(), ManyError> {
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

impl ledger::LedgerTransactionsModuleBackend for LedgerModuleImpl {
    fn transactions(
        &self,
        _args: ledger::TransactionsArgs,
    ) -> Result<ledger::TransactionsReturns, ManyError> {
        Ok(ledger::TransactionsReturns {
            nb_transactions: self.storage.nb_transactions(),
        })
    }

    fn list(&self, args: ledger::ListArgs) -> Result<ledger::ListReturns, ManyError> {
        let ledger::ListArgs {
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
                .map_err(|e| ManyError::deserialization_error(e.to_string()))
        }));

        let iter = filter_account(iter, filter.account);
        let iter = filter_transaction_kind(iter, filter.kind);
        let iter = filter_symbol(iter, filter.symbol);
        let iter = filter_date(iter, filter.date_range.unwrap_or_default());

        let transactions: Vec<Transaction> = iter.take(count).collect::<Result<_, _>>()?;

        Ok(ledger::ListReturns {
            nb_transactions,
            transactions,
        })
    }
}

// This module is always supported, but will only be added when created using an ABCI
// flag.
impl ManyAbciModuleBackend for LedgerModuleImpl {
    #[rustfmt::skip]
    fn init(&mut self) -> Result<AbciInit, ManyError> {
        Ok(AbciInit {
            endpoints: BTreeMap::from([
                ("ledger.info".to_string(), EndpointInfo { is_command: false }),
                ("ledger.balance".to_string(), EndpointInfo { is_command: false }),
                ("ledger.send".to_string(), EndpointInfo { is_command: true }),
                ("ledger.transactions".to_string(), EndpointInfo { is_command: false }),
                ("ledger.list".to_string(), EndpointInfo { is_command: false }),
            ]),
        })
    }

    fn init_chain(&mut self) -> Result<(), ManyError> {
        info!("abci.init_chain()",);
        Ok(())
    }

    fn begin_block(&mut self, info: AbciBlock) -> Result<(), ManyError> {
        let time = info.time;
        info!("abci.block_begin(): time={:?}", time);

        if let Some(time) = time {
            let time = UNIX_EPOCH.checked_add(Duration::from_secs(time)).unwrap();
            self.storage.set_time(time);
        }

        Ok(())
    }

    fn info(&self) -> Result<AbciInfo, ManyError> {
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

    fn commit(&mut self) -> Result<AbciCommitInfo, ManyError> {
        let result = self.storage.commit();

        info!(
            "abci.commit(): retain_height={} hash={}",
            result.retain_height,
            hex::encode(result.hash.as_slice()).as_str()
        );
        Ok(result)
    }

    fn list_snapshots(&mut self) -> Result<AbciListSnapshot, ManyError> {
        let result = self.storage.list_snapshots();
        info!(
            "abci.list_snapshots(): Snapshot={:?}",
            result.all_snapshots,
        );
        Ok(result)
    }
}
