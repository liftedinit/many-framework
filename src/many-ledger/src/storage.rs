use crate::migration::{LedgerMigrations, MIGRATIONS, MIGRATIONS_KEY};
use crate::storage::event::HEIGHT_EVENTID_SHIFT;
use many_error::ManyError;
use many_identity::Address;
use many_migration::Migration;
use many_modules::events::EventId;
use many_types::ledger::{Symbol, TokenAmount};
use many_types::Timestamp;
use merk::{rocksdb, BatchEntry, Op};
use std::collections::BTreeMap;
use std::path::Path;
use tracing::info;

mod abci;
mod account;
pub mod data;
pub mod event;
mod idstore;
mod ledger;
mod ledger_commands;
pub mod multisig;

pub(super) fn key_for_account_balance(id: &Address, symbol: &Symbol) -> Vec<u8> {
    format!("/balances/{id}/{symbol}").into_bytes()
}

pub struct LedgerStorage {
    symbols: BTreeMap<Symbol, String>,
    persistent_store: merk::Merk,

    /// When this is true, we do not commit every transactions as they come,
    /// but wait for a `commit` call before committing the batch to the
    /// persistent store.
    blockchain: bool,

    latest_tid: EventId,

    current_time: Option<Timestamp>,
    current_hash: Option<Vec<u8>>,

    next_account_id: u32,
    account_identity: Address,

    migrations: LedgerMigrations,
}

impl LedgerStorage {
    #[cfg(feature = "balance_testing")]
    pub(crate) fn set_balance_only_for_testing(
        &mut self,
        account: Address,
        amount: u64,
        symbol: Address,
    ) {
        assert!(self.symbols.contains_key(&symbol));
        // Make sure we don't run this function when the store has started.
        assert_eq!(self.current_hash, None);

        let key = key_for_account_balance(&account, &symbol);
        let amount = TokenAmount::from(amount);

        self.persistent_store
            .apply(&[(key, Op::Put(amount.to_vec()))])
            .unwrap();

        // Always commit to the store. In blockchain mode this will fail.
        self.persistent_store.commit(&[]).unwrap();
    }
}

impl std::fmt::Debug for LedgerStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("LedgerStorage")
            .field("symbols", &self.symbols)
            .field("migrations", &self.migrations)
            .finish()
    }
}

impl LedgerStorage {
    #[inline]
    pub fn set_time(&mut self, time: Timestamp) {
        self.current_time = Some(time);
    }
    #[inline]
    pub fn now(&self) -> Timestamp {
        self.current_time.unwrap_or_else(Timestamp::now)
    }

    pub(crate) fn add_migrations(&mut self, mut migrations: LedgerMigrations) {
        self.migrations.append(&mut migrations);
        self.persistent_store
            .apply(&[(
                MIGRATIONS_KEY.to_vec(),
                Op::Put(
                    minicbor::to_vec(migrations.values().collect::<Vec<&Migration<'_, _, _>>>())
                        .unwrap(),
                ),
            )])
            .unwrap();
    }

    pub fn load<P: AsRef<Path>>(persistent_path: P, blockchain: bool) -> Result<Self, String> {
        let persistent_store = merk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

        let symbols = persistent_store
            .get(b"/config/symbols")
            .map_err(|e| e.to_string())?;
        let symbols: BTreeMap<Symbol, String> = symbols
            .map_or_else(|| Ok(Default::default()), |bytes| minicbor::decode(&bytes))
            .map_err(|e| e.to_string())?;
        let next_account_id = persistent_store
            .get(b"/config/account_id")
            .unwrap()
            .map_or(0, |x| {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(x.as_slice());
                u32::from_be_bytes(bytes)
            });

        let account_identity: Address = Address::from_bytes(
            &persistent_store
                .get(b"/config/identity")
                .expect("Could not open storage.")
                .expect("Could not find key '/config/identity' in storage."),
        )
        .map_err(|e| e.to_string())?;

        let height = persistent_store.get(b"/height").unwrap().map_or(0u64, |x| {
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(x.as_slice());
            u64::from_be_bytes(bytes)
        });

        let latest_tid = EventId::from(height << HEIGHT_EVENTID_SHIFT);

        let all_migrations = persistent_store
            .get(MIGRATIONS_KEY)
            .expect("Could not open storage.")
            .map_or(LedgerMigrations::new(), |x| {
                minicbor::decode_with::<_, Vec<Migration<_, _>>>(&x, &mut MIGRATIONS.clone())
                    .unwrap()
                    .into_iter()
                    .map(|mig| (mig.migration.name(), mig))
                    .collect::<LedgerMigrations>()
            });

        info!("LedgerMigrations list");
        for migration in all_migrations.values() {
            info!("{migration}")
        }

        Ok(Self {
            symbols,
            persistent_store,
            blockchain,
            latest_tid,
            current_time: None,
            current_hash: None,
            next_account_id,
            account_identity,
            migrations: all_migrations,
        })
    }

    pub fn new<P: AsRef<Path>>(
        symbols: BTreeMap<Symbol, String>,
        initial_balances: BTreeMap<Address, BTreeMap<Symbol, TokenAmount>>,
        persistent_path: P,
        identity: Address,
        blockchain: bool,
        maybe_seed: Option<u64>,
        maybe_keys: Option<BTreeMap<Vec<u8>, Vec<u8>>>,
    ) -> Result<Self, String> {
        let mut persistent_store = merk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

        let mut batch: Vec<BatchEntry> = Vec::new();

        for (k, v) in initial_balances.into_iter() {
            for (symbol, tokens) in v.into_iter() {
                if !symbols.contains_key(&symbol) {
                    return Err(format!(r#"Unknown symbol "{symbol}" for identity {k}"#));
                }

                let key = key_for_account_balance(&k, &symbol);
                batch.push((key, Op::Put(tokens.to_vec())));
            }
        }

        batch.push((b"/config/identity".to_vec(), Op::Put(identity.to_vec())));
        batch.push((
            b"/config/symbols".to_vec(),
            Op::Put(minicbor::to_vec(&symbols).map_err(|e| e.to_string())?),
        ));

        persistent_store
            .apply(batch.as_slice())
            .map_err(|e| e.to_string())?;

        // Apply keys and seed.
        if let Some(seed) = maybe_seed {
            persistent_store
                .apply(&[(
                    b"/config/idstore_seed".to_vec(),
                    Op::Put(seed.to_be_bytes().to_vec()),
                )])
                .unwrap();
        }
        if let Some(keys) = maybe_keys {
            for (k, v) in keys {
                persistent_store.apply(&[(k, Op::Put(v))]).unwrap();
            }
        }

        persistent_store.commit(&[]).map_err(|e| e.to_string())?;

        Ok(Self {
            symbols,
            persistent_store,
            blockchain,
            latest_tid: EventId::from(vec![0]),
            current_time: None,
            current_hash: None,
            next_account_id: 0,
            account_identity: identity,
            migrations: LedgerMigrations::new(),
        })
    }

    pub fn commit_persistent_store(&mut self) -> Result<(), String> {
        self.persistent_store.commit(&[]).map_err(|e| e.to_string())
    }

    pub fn get_symbols(&self) -> BTreeMap<Symbol, String> {
        self.symbols.clone()
    }

    fn inc_height(&mut self) -> u64 {
        let current_height = self.get_height();
        self.persistent_store
            .apply(&[(
                b"/height".to_vec(),
                Op::Put((current_height + 1).to_be_bytes().to_vec()),
            )])
            .unwrap();
        current_height
    }

    /// Return the current height of the blockchain.
    /// The current height correspond to finished, committed blocks.
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

    pub fn hash(&self) -> Vec<u8> {
        self.current_hash
            .as_ref()
            .map_or_else(|| self.persistent_store.root_hash().to_vec(), |x| x.clone())
    }

    pub fn block_hotfix<
        T: minicbor::Encode<()>,
        C: for<'a> minicbor::Decode<'a, ()>,
        F: FnOnce() -> T,
    >(
        &self,
        name: &str,
        data: F,
    ) -> Result<Option<C>, ManyError> {
        if let Some(migration) = self.migrations.get(name) {
            // We are building the current block so the correct height is the current height (finished blocks) + 1
            if self.get_height() + 1 == migration.metadata.block_height && migration.is_enabled() {
                let data_enc = minicbor::to_vec(data()).map_err(ManyError::serialization_error)?;
                let new_data = migration.hotfix(&data_enc, self.get_height() + 1);
                if let Some(new_data) = new_data {
                    return Ok(Some(
                        minicbor::decode(&new_data).map_err(ManyError::deserialization_error)?,
                    ));
                }
                return Err(ManyError::unknown(
                    "Something went wrong while running migration \"{name}\"",
                ));
            }
            return Ok(None);
        }
        Ok(None)
    }
}
