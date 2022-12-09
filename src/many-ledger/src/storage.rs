use crate::json::SymbolMetaJson;
use crate::migration::tokens::TOKEN_MIGRATION;
use crate::migration::{LedgerMigrations, MIGRATIONS};
use crate::storage::event::HEIGHT_EVENTID_SHIFT;
use crate::storage::iterator::LedgerIterator;
use crate::storage::ledger_tokens::{key_for_ext_info, key_for_symbol, SYMBOL_ROOT};
use many_error::ManyError;
use many_identity::Address;
use many_migration::{MigrationConfig, MigrationSet};
use many_modules::events::EventId;
use many_modules::ledger::extended_info::TokenExtendedInfo;
use many_types::ledger::{Symbol, TokenAmount, TokenInfo, TokenInfoSummary, TokenInfoSupply};
use many_types::{SortOrder, Timestamp};
use merk::{BatchEntry, Op};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::str::FromStr;

mod abci;
mod account;
pub mod data;
pub mod event;
mod idstore;
pub mod iterator;
mod ledger;
mod ledger_commands;
pub mod ledger_tokens;
pub mod multisig;

pub(super) fn key_for_account_balance(id: &Address, symbol: &Symbol) -> Vec<u8> {
    format!("/balances/{id}/{symbol}").into_bytes()
}

pub struct LedgerStorage {
    persistent_store: merk::Merk,

    /// When this is true, we do not commit every transactions as they come,
    /// but wait for a `commit` call before committing the batch to the
    /// persistent store.
    blockchain: bool,

    latest_tid: EventId,

    current_time: Option<Timestamp>,
    current_hash: Option<Vec<u8>>,

    next_subresource: u32,
    root_identity: Address,

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
        assert!(self
            .get_symbols()
            .expect("Error while loading symbols")
            .contains(&symbol));
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

    pub fn migrations(&self) -> &LedgerMigrations {
        &self.migrations
    }

    fn subresource_db_key(migration_is_active: bool) -> Vec<u8> {
        if migration_is_active {
            b"/config/subresource_id".to_vec()
        } else {
            b"/config/account_id".to_vec()
        }
    }

    pub fn new_subresource_id(&mut self) -> Address {
        let current_id = self.next_subresource;
        self.next_subresource += 1;
        self.persistent_store
            .apply(&[(
                LedgerStorage::subresource_db_key(self.migrations.is_active(&TOKEN_MIGRATION)),
                Op::Put(self.next_subresource.to_be_bytes().to_vec()),
            )])
            .unwrap();

        self.root_identity
            .with_subresource_id(current_id)
            .expect("Too many subresources")
    }

    pub fn load<P: AsRef<Path>>(
        persistent_path: P,
        blockchain: bool,
        migration_config: Option<MigrationConfig>,
    ) -> Result<Self, String> {
        let persistent_store = merk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

        let root_identity: Address = Address::from_bytes(
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

        // The call to `saturating_sub()` is required to fix
        // https://github.com/liftedinit/many-framework/issues/289
        //
        // The `commit()` function computes the `latest_tid` using the previous height while
        // the following line computes the `latest_tid` using the current height.
        //
        // The discrepancy will lead to an application hash mismatch if the block following the `load()` contains
        // a transaction.
        let latest_tid = EventId::from(height.saturating_sub(1) << HEIGHT_EVENTID_SHIFT);
        let migrations = migration_config.map_or_else(MigrationSet::empty, |config| {
            LedgerMigrations::load(&MIGRATIONS, config, height)
        })?;

        let next_subresource = persistent_store
            .get(&LedgerStorage::subresource_db_key(
                migrations.is_active(&TOKEN_MIGRATION),
            ))
            .unwrap()
            .map_or(0, |x| {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(x.as_slice());
                u32::from_be_bytes(bytes)
            });

        Ok(Self {
            persistent_store,
            blockchain,
            latest_tid,
            current_time: None,
            current_hash: None,
            next_subresource,
            root_identity,
            migrations,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new<P: AsRef<Path>>(
        symbols: BTreeMap<Symbol, String>,
        symbols_meta: Option<BTreeMap<Symbol, SymbolMetaJson>>, // TODO: This is dumb, don't do that
        initial_balances: BTreeMap<Address, BTreeMap<Symbol, TokenAmount>>,
        persistent_path: P,
        identity: Address,
        blockchain: bool,
        maybe_seed: Option<u64>,
        maybe_keys: Option<BTreeMap<Vec<u8>, Vec<u8>>>,
        migration_config: Option<MigrationConfig>,
    ) -> Result<Self, String> {
        let mut persistent_store = merk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

        // NOTE: Migrations are only applied in blockchain mode when loading an existing DB
        //       It is currently NOT possible to run new code in non-blockchain mode when loading an existing DB
        let migrations = migration_config.map_or_else(MigrationSet::empty, |config| {
            LedgerMigrations::load(&MIGRATIONS, config, 0)
        })?;

        let mut batch: Vec<BatchEntry> = Vec::new();
        let mut total_supply = BTreeMap::new();
        for (k, v) in initial_balances.into_iter() {
            for (symbol, tokens) in v.into_iter() {
                if let Some(x) = total_supply.get_mut(&symbol) {
                    *x += tokens.clone(); // TODO: Remove clone
                } else {
                    total_supply.insert(symbol, tokens.clone());
                }

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

        // If the Token Migration is active and some symbol metadata were provided, we can add those to the storage
        // Note: This will change storage hash
        if migrations.is_active(&TOKEN_MIGRATION) {
            if let Some(symbols_meta) = symbols_meta {
                for ((s1, ticker), (s2, meta)) in symbols.iter().zip(symbols_meta.into_iter()) {
                    if s1 != &s2 {
                        return Err("Symbols {s1} and {s2} don't match".to_string());
                    }

                    let total_supply = total_supply.get(s1).expect("Unable to get total supply");
                    let info = TokenInfo {
                        symbol: s2,
                        summary: TokenInfoSummary {
                            name: meta.name,
                            ticker: ticker.clone(),
                            decimals: meta.decimals,
                        },
                        supply: TokenInfoSupply {
                            total: total_supply.clone(),
                            circulating: total_supply.clone(),
                            maximum: meta.maximum,
                        },
                        owner: meta.owner,
                    };

                    batch.push((
                        key_for_ext_info(s1),
                        Op::Put(
                            minicbor::to_vec(TokenExtendedInfo::default())
                                .map_err(|e| e.to_string())?,
                        ),
                    ));
                    batch.push((
                        key_for_symbol(s1),
                        Op::Put(minicbor::to_vec(info).map_err(|e| e.to_string())?),
                    ));
                }
            }
        }

        // Apply keys and seed.
        if let Some(seed) = maybe_seed {
            batch.push((
                b"/config/idstore_seed".to_vec(),
                Op::Put(seed.to_be_bytes().to_vec()),
            ));
        }
        if let Some(keys) = maybe_keys {
            for (k, v) in keys {
                batch.push((k, Op::Put(v)));
            }
        }

        // Batch keys need to be sorted
        batch.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));

        persistent_store
            .apply(batch.as_slice())
            .map_err(|e| e.to_string())?;

        persistent_store.commit(&[]).map_err(|e| e.to_string())?;

        Ok(Self {
            persistent_store,
            blockchain,
            latest_tid: EventId::from(vec![0]),
            current_time: None,
            current_hash: None,
            next_subresource: 0,
            root_identity: identity,
            migrations,
        })
    }

    pub fn commit_persistent_store(&mut self) -> Result<(), String> {
        self.persistent_store.commit(&[]).map_err(|e| e.to_string())
    }

    /// Fetch symbol and ticker from '/config/symbols/.
    /// Kept for backward compatibility
    pub fn get_symbols_and_tickers(&self) -> Result<BTreeMap<Symbol, String>, ManyError> {
        minicbor::decode::<BTreeMap<Symbol, String>>(
            &self
                .persistent_store
                .get(b"/config/symbols")
                .map_err(ManyError::unknown)? // TODO: Custom error
                .ok_or_else(|| ManyError::unknown("No symbol data found"))?, // TODO: Custom error
        )
        .map_err(ManyError::deserialization_error)
    }

    /// Fetch symbols from `/config/symbols/{symbol}` iif "Token Migration" is enabled
    ///     No CBOR decoding needed.
    /// Else symbols are fetched using the legacy method via `get_symbols_and_tickers()`
    pub fn get_symbols(&self) -> Result<BTreeSet<Symbol>, ManyError> {
        let mut symbols = BTreeSet::new();
        if self.migrations.is_active(&TOKEN_MIGRATION) {
            let it = LedgerIterator::all_symbols(&self.persistent_store, SortOrder::Indeterminate);
            for item in it {
                let (k, _) = item.map_err(|e| ManyError::unknown(e.to_string()))?;
                symbols.insert(Symbol::from_str(
                    std::str::from_utf8(&k.as_ref()[SYMBOL_ROOT.len()..])
                        .map_err(ManyError::deserialization_error)?, // TODO: We could safely use from_utf8_unchecked() if performance is an issue
                )?);
            }
        } else {
            let symbols_and_tickers = self.get_symbols_and_tickers()?;
            symbols.extend(symbols_and_tickers.keys())
        }
        Ok(symbols)
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
        &mut self,
        name: &str,
        data: F,
    ) -> Result<Option<C>, ManyError> {
        let data_enc = minicbor::to_vec(data()).map_err(ManyError::serialization_error)?;

        if let Some(data) = self
            .migrations
            .hotfix(name, &data_enc, self.get_height() + 1)?
        {
            let dec_data = minicbor::decode(&data).map_err(ManyError::deserialization_error)?;
            Ok(Some(dec_data))
        } else {
            Ok(None)
        }
    }
}
