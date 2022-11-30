use crate::error;
use crate::json::InitialStateJson;
use crate::storage::LedgerStorage;
use many_error::ManyError;
use many_migration::MigrationConfig;
use std::fmt::Debug;
use std::path::Path;
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
pub struct LedgerModuleImpl {
    storage: LedgerStorage,
}

impl LedgerModuleImpl {
    pub fn new<P: AsRef<Path>>(
        state: InitialStateJson,
        migration_config: Option<MigrationConfig>,
        persistence_store_path: P,
        blockchain: bool,
    ) -> Result<Self, ManyError> {
        let mut storage = LedgerStorage::new(
            state.symbols(),
            state.balances()?,
            persistence_store_path,
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
            migration_config,
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

        if let Some(h) = state.hash {
            // Verify the hash.
            let actual = hex::encode(storage.hash());
            if actual != h {
                return Err(error::invalid_initial_state(h, actual));
            }
        }

        info!(
            height = storage.get_height(),
            hash = hex::encode(storage.hash()).as_str()
        );

        tracing::debug!("Final migrations: {:?}", storage.migrations());

        Ok(Self { storage })
    }

    pub fn load<P: AsRef<Path>>(
        migrations: Option<MigrationConfig>,
        persistence_store_path: P,
        blockchain: bool,
    ) -> Result<Self, ManyError> {
        let storage = LedgerStorage::load(persistence_store_path, blockchain, migrations).unwrap();

        tracing::debug!("Final migrations: {:?}", storage.migrations());

        Ok(Self { storage })
    }

    #[cfg(feature = "balance_testing")]
    pub fn set_balance_only_for_testing(
        &mut self,
        account: many_identity::Address,
        balance: u64,
        symbol: many_types::ledger::Symbol,
    ) {
        self.storage
            .set_balance_only_for_testing(account, balance, symbol);
    }
}
