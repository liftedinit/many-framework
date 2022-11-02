use crate::json::InitialStateJson;
// TODO: MIGRATION
// use crate::migration::Migration;
use crate::migration::MIGRATIONS;
use crate::{error, storage::LedgerStorage};
use many_error::ManyError;
use many_identity::Address;
use many_migration::{load_enable_all_regular_migrations, load_migrations, Migration};
use many_types::ledger::Symbol;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use tracing::info;

mod abci;
pub mod account;
pub mod allow_addrs;
mod data;
mod event;
mod idstore;
pub mod idstore_webauthn;
mod ledger;
mod ledger_commands;
mod multisig;

/// A simple ledger that keeps transactions in memory.
#[derive(Debug)]
pub struct LedgerModuleImpl<'a> {
    storage: LedgerStorage<'a>,
}

impl<'a> LedgerModuleImpl<'a> {
    pub fn new<P: AsRef<Path>>(
        initial_state: Option<InitialStateJson>,
        migrations_config: Option<PathBuf>,
        persistence_store_path: P,
        blockchain: bool,
    ) -> Result<Self, ManyError> {
        let storage = if let Some(state) = initial_state {
            let migrations = load_enable_all_regular_migrations(&MIGRATIONS);
            info!("Loading and enabling all regular migrations");
            for migration in migrations.values() {
                info!("{migration}")
            }

            let mut storage = LedgerStorage::new(
                state.symbols(),
                state.balances()?,
                persistence_store_path,
                migrations,
                state.identity,
                blockchain,
                state.id_store_seed,
                state.id_store_keys.map(|keys| {
                    keys.iter()
                        .map(|(k, v)| {
                            let k = base64::decode(k).expect("Invalid base64 for key");
                            let v = base64::decode(v).expect("Invalid base64 for value");
                            (k, v)
                        })
                        .collect()
                }),
            )
            .map_err(ManyError::unknown)?;

            if let Some(accounts) = state.accounts {
                for account in accounts {
                    account
                        .create_account(&mut storage)
                        .expect("Could not create accounts");
                }
                storage.commit_persistent_store().expect("Could not commit");
            }
            // if let Some(h) = state.hash {
            //     // Verify the hash.
            //     let actual = hex::encode(storage.hash());
            //     if actual != h {
            //         return Err(error::invalid_initial_state(h, actual));
            //     }
            // }

            storage
        } else {
            let migrations = migrations_config
                .map(|file| {
                    let content = std::fs::read_to_string(file)
                        .expect("Could not read file passed to --migrations_config");
                    load_migrations(&MIGRATIONS, &content).expect("Could not read migration file")
                })
                .unwrap_or(BTreeMap::new());

            info!("Loaded migrations from configuration file");
            for migration in migrations.values() {
                info!("{migration}")
            }

            LedgerStorage::load(persistence_store_path, migrations, blockchain).unwrap()
        };

        info!(
            height = storage.get_height(),
            hash = hex::encode(storage.hash()).as_str()
        );

        Ok(Self { storage })
    }

    #[cfg(feature = "balance_testing")]
    pub fn set_balance_only_for_testing(&mut self, account: Address, balance: u64, symbol: Symbol) {
        self.storage
            .set_balance_only_for_testing(account, balance, symbol);
    }
}
