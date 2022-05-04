use crate::error;
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use many::server::module::abci_backend::{AbciCommitInfo, AbciListSnapshot, Snapshot};
use many::types::ledger::{Symbol, TokenAmount, Transaction, TransactionId};
use many::types::{CborRange, SortOrder};
use many::{Identity, ManyError};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, Bound};
use std::fs::{self, File};
use std::ops::RangeBounds;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::info;

pub(crate) const TRANSACTIONS_ROOT: &[u8] = b"/transactions/";

// Left-shift the height by this amount of bits
const HEIGHT_TXID_SHIFT: u64 = 32;

/// Returns the key for the persistent kv-store.
pub(crate) fn key_for_account(id: &Identity, symbol: &Symbol) -> Vec<u8> {
    format!("/balances/{}/{}", id, symbol).into_bytes()
}

pub(crate) fn key_for_transaction(id: TransactionId) -> Vec<u8> {
    vec![TRANSACTIONS_ROOT.to_vec(), id.into()].concat()
}

pub struct LedgerStorage {
    symbols: BTreeMap<Symbol, String>,
    persistent_store: fmerk::Merk,
    snapshots: PathBuf,

    /// When this is true, we do not commit every transactions as they come,
    /// but wait for a `commit` call before committing the batch to the
    /// persistent store.
    blockchain: bool,

    latest_tid: TransactionId,

    current_time: Option<SystemTime>,
    current_hash: Option<Vec<u8>>,
}

impl std::fmt::Debug for LedgerStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LedgerStorage")
            .field("symbols", &self.symbols)
            .finish()
    }
}

impl LedgerStorage {
    pub fn set_time(&mut self, time: SystemTime) {
        self.current_time = Some(time);
    }

    pub fn load<P: AsRef<Path>>(
        persistent_path: P,
        snapshot_path: P,
        blockchain: bool,
    ) -> Result<Self, String> {
        let snapshots = snapshot_path.as_ref().to_path_buf();
        let persistent_store = fmerk::Merk::open(persistent_path).map_err(|e| e.to_string())?;
        let symbols = persistent_store
            .get(b"/config/symbols")
            .map_err(|e| e.to_string())?;
        let symbols: BTreeMap<Symbol, String> = symbols
            .map_or_else(|| Ok(Default::default()), |bytes| minicbor::decode(&bytes))
            .map_err(|e| e.to_string())?;

        let height = persistent_store.get(b"/height").unwrap().map_or(0u64, |x| {
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(x.as_slice());
            u64::from_be_bytes(bytes)
        });

        let latest_tid = TransactionId::from(height << HEIGHT_TXID_SHIFT);

        Ok(Self {
            symbols,
            persistent_store,
            snapshots,
            blockchain,
            latest_tid,
            current_time: None,
            current_hash: None,
        })
    }

    pub fn new<P: AsRef<Path>>(
        symbols: BTreeMap<Symbol, String>,
        initial_balances: BTreeMap<Identity, BTreeMap<Symbol, TokenAmount>>,
        persistent_path: P,
        snapshot_path: P,
        blockchain: bool,
    ) -> Result<Self, String> {
        let snapshots = snapshot_path.as_ref().to_path_buf();

        let mut persistent_store = fmerk::Merk::open(persistent_path).map_err(|e| e.to_string())?;
        let mut batch: Vec<fmerk::BatchEntry> = Vec::new();

        for (k, v) in initial_balances.into_iter() {
            for (symbol, tokens) in v.into_iter() {
                if !symbols.contains_key(&symbol) {
                    return Err(format!(r#"Unknown symbol "{}" for identity {}"#, symbol, k));
                }

                let key = key_for_account(&k, &symbol);
                batch.push((key, fmerk::Op::Put(tokens.to_vec())));
            }
        }

        batch.push((
            b"/config/symbols".to_vec(),
            fmerk::Op::Put(minicbor::to_vec(&symbols).map_err(|e| e.to_string())?),
        ));

        persistent_store
            .apply(batch.as_slice())
            .map_err(|e| e.to_string())?;
        persistent_store.commit(&[]).map_err(|e| e.to_string())?;

        Ok(Self {
            symbols,
            persistent_store,
            snapshots,
            blockchain,
            latest_tid: TransactionId::from(vec![0]),
            current_time: None,
            current_hash: None,
        })
    }

    pub fn get_symbols(&self) -> BTreeMap<Symbol, String> {
        self.symbols.clone()
    }

    fn inc_height(&mut self) -> u64 {
        let current_height = self.get_height();
        self.persistent_store
            .apply(&[(
                b"/height".to_vec(),
                fmerk::Op::Put((current_height + 1).to_be_bytes().to_vec()),
            )])
            .unwrap();
        current_height
    }

    pub fn get_height(&self) -> u64 {
        self.persistent_store
            .get(b"/height")
            .unwrap()
            .map_or(0u64, |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                u64::from_be_bytes(bytes)
            })
    }

    fn new_transaction_id(&mut self) -> TransactionId {
        self.latest_tid += 1;
        self.latest_tid.clone()
    }

    pub fn create_snapshot(&self, height: u64) -> Result<Snapshot, ManyError> {
        let dnt = chrono::Utc::now().date();
        let snapshot_name = format!("{}-{}-{}", height, "snapshot", dnt);
        self.persistent_store
            .snapshot(self.snapshots.join(&snapshot_name))
            .map_err(|e| ManyError::snapshot_creation_error(e.to_string()))?;

        let gz = File::create(
            self.snapshots
                .join(format!("many-ledger-snapshot-{}.tar.gz", height)),
        )
        .map_err(|e| ManyError::snapshot_not_found(e.to_string()))?;

        let encoder = GzEncoder::new(gz, Compression::fast());

        let mut tar = tar::Builder::new(encoder);
        tar.append_dir_all(&snapshot_name, self.snapshots.join(&snapshot_name))
            .map_err(|e| ManyError::snapshot_dir_error(e.to_string()))?;

        fs::remove_dir_all(self.snapshots.join(&snapshot_name).as_path())
            .map_err(|e| ManyError::snapshot_dir_error(e.to_string()))?;

        let hash = self.persistent_store.root_hash().to_vec();
        let path = self
            .snapshots
            .join(format!("many-ledger-snapshot-{}.tar.gz", height));

        Ok(Snapshot { path, height, hash })
    }

    pub fn get_snapshot(&self, height: u64) -> Result<Snapshot, ManyError> {
        let snapshot_name = format!("many-ledger-snapshot-{}.tar.gz", height);
        let path = self.snapshots.join(&snapshot_name);
        let hash = self.persistent_store.root_hash().to_vec();

        Ok(Snapshot { path, height, hash })
    }

    pub fn list_snapshots(&mut self) -> AbciListSnapshot {
        let all = self.get_snapshot(self.get_height()).unwrap();
        let mut snapshots = Vec::new();
        for i in 0..=all.height {
            let snapshot = self.get_snapshot(i).unwrap();
            snapshots.push(snapshot);
        }
        AbciListSnapshot {
            all_snapshots: snapshots,
        }
    }

    pub fn commit(&mut self) -> AbciCommitInfo {
        let height = self.inc_height();
        let retain_height = 0;
        self.persistent_store.commit(&[]).unwrap();

        let hash = self.persistent_store.root_hash().to_vec();
        self.current_hash = Some(hash.clone());

        self.latest_tid = TransactionId::from(height << HEIGHT_TXID_SHIFT);

        if height % 1000 == 0 {
            if let Err(e) = self.create_snapshot(height) {
                tracing::error!("snapshot error: {}", e.to_string());
            }
        }

        AbciCommitInfo {
            retain_height,
            hash: hash.into(),
        }
    }

    pub fn nb_transactions(&self) -> u64 {
        self.persistent_store
            .get(b"/transactions_count")
            .unwrap()
            .map_or(0, |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                u64::from_be_bytes(bytes)
            })
    }

    fn add_transaction(&mut self, transaction: Transaction) {
        let current_nb_transactions = self.nb_transactions();

        self.persistent_store
            .apply(&[
                (
                    key_for_transaction(transaction.id.clone()),
                    fmerk::Op::Put(minicbor::to_vec(&transaction).unwrap()),
                ),
                (
                    b"/transactions_count".to_vec(),
                    fmerk::Op::Put((current_nb_transactions + 1).to_be_bytes().to_vec()),
                ),
            ])
            .unwrap();
    }

    pub fn get_balance(&self, identity: &Identity, symbol: &Symbol) -> TokenAmount {
        if identity.is_anonymous() {
            TokenAmount::zero()
        } else {
            let key = key_for_account(identity, symbol);
            match self.persistent_store.get(&key).unwrap() {
                None => TokenAmount::zero(),
                Some(amount) => TokenAmount::from(amount),
            }
        }
    }

    fn get_all_balances(&self, identity: &Identity) -> BTreeMap<&Symbol, TokenAmount> {
        if identity.is_anonymous() {
            // Anonymous cannot hold funds.
            BTreeMap::new()
        } else {
            let mut result = BTreeMap::new();
            for symbol in self.symbols.keys() {
                match self
                    .persistent_store
                    .get(&key_for_account(identity, symbol))
                {
                    Ok(None) => {}
                    Ok(Some(value)) => {
                        result.insert(symbol, TokenAmount::from(value));
                    }
                    Err(_) => {}
                }
            }

            result
        }
    }

    pub fn get_multiple_balances(
        &self,
        identity: &Identity,
        symbols: &BTreeSet<Symbol>,
    ) -> BTreeMap<&Symbol, TokenAmount> {
        if symbols.is_empty() {
            self.get_all_balances(identity)
        } else {
            self.get_all_balances(identity)
                .into_iter()
                .filter(|(k, _v)| symbols.contains(*k))
                .collect()
        }
    }

    pub fn send(
        &mut self,
        from: &Identity,
        to: &Identity,
        symbol: &Symbol,
        amount: TokenAmount,
    ) -> Result<(), ManyError> {
        if amount.is_zero() || from == to {
            // NOOP.
            return Ok(());
        }
        if to.is_anonymous() || from.is_anonymous() {
            return Err(error::anonymous_cannot_hold_funds());
        }

        let mut amount_from = self.get_balance(from, symbol);
        if amount > amount_from {
            return Err(error::insufficient_funds());
        }

        info!("send({} => {}, {} {})", from, to, &amount, symbol);

        let mut amount_to = self.get_balance(to, symbol);
        amount_to += amount.clone();
        amount_from -= amount.clone();

        // Keys in batch must be sorted.
        let key_from = key_for_account(from, symbol);
        let key_to = key_for_account(to, symbol);

        let batch: Vec<fmerk::BatchEntry> = match key_from.cmp(&key_to) {
            Ordering::Less | Ordering::Equal => vec![
                (key_from, fmerk::Op::Put(amount_from.to_vec())),
                (key_to, fmerk::Op::Put(amount_to.to_vec())),
            ],
            _ => vec![
                (key_to, fmerk::Op::Put(amount_to.to_vec())),
                (key_from, fmerk::Op::Put(amount_from.to_vec())),
            ],
        };

        self.persistent_store.apply(&batch).unwrap();

        let id = self.new_transaction_id();

        self.add_transaction(Transaction::send(
            id,
            self.current_time.unwrap_or_else(SystemTime::now),
            *from,
            *to,
            symbol.to_string(),
            amount,
        ));

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }

        Ok(())
    }

    pub fn hash(&self) -> Vec<u8> {
        self.current_hash
            .as_ref()
            .map_or_else(|| self.persistent_store.root_hash().to_vec(), |x| x.clone())
    }

    pub fn iter(&self, range: CborRange<TransactionId>, order: SortOrder) -> LedgerIterator {
        LedgerIterator::scoped_by_id(&self.persistent_store, range, order)
    }
}

pub struct LedgerIterator<'a> {
    inner: fmerk::rocksdb::DBIterator<'a>,
}

impl<'a> LedgerIterator<'a> {
    pub fn scoped_by_id(
        merk: &'a fmerk::Merk,
        range: CborRange<TransactionId>,
        order: SortOrder,
    ) -> Self {
        use fmerk::rocksdb::{IteratorMode, ReadOptions};
        let mut opts = ReadOptions::default();

        match range.start_bound() {
            Bound::Included(x) => opts.set_iterate_lower_bound(key_for_transaction(x.clone())),
            Bound::Excluded(x) => opts.set_iterate_lower_bound(key_for_transaction(x.clone() + 1)),
            Bound::Unbounded => opts.set_iterate_lower_bound(TRANSACTIONS_ROOT),
        }
        match range.end_bound() {
            Bound::Included(x) => opts.set_iterate_upper_bound(key_for_transaction(x.clone() + 1)),
            Bound::Excluded(x) => opts.set_iterate_upper_bound(key_for_transaction(x.clone())),
            Bound::Unbounded => {
                let mut bound = TRANSACTIONS_ROOT.to_vec();
                bound[TRANSACTIONS_ROOT.len() - 1] += 1;
                opts.set_iterate_upper_bound(bound);
            }
        }

        let mode = match order {
            SortOrder::Indeterminate | SortOrder::Ascending => IteratorMode::Start,
            SortOrder::Descending => IteratorMode::End,
        };

        Self {
            inner: merk.iter_opt(mode, opts),
        }
    }
}

impl<'a> Iterator for LedgerIterator<'a> {
    type Item = (Box<[u8]>, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(k, v)| {
            let new_v = fmerk::tree::Tree::decode(k.to_vec(), v.as_ref());

            (k, new_v.value().to_vec())
        })
    }
}
