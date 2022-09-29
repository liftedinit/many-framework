pub mod data;

use data::initial_metrics_data;
use merk::Op;
use minicbor::{Decode, Encode};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(feature = "migrate_blocks")]
use many_protocol::ResponseMessage;
use serde::{Deserialize, Serialize};

use crate::storage::MIGRATIONS_KEY;

#[cfg(feature = "block_9400")]
mod block_9400;

#[cfg(feature = "migrate_blocks")]
pub fn migrate(tx_id: &[u8], response: ResponseMessage) -> ResponseMessage {
    match hex::encode(tx_id).as_str() {
        #[cfg(feature = "block_9400")]
        "241e00000001" => block_9400::migrate(response),
        _ => response,
    }
}

pub type MigrationMap = BTreeMap<MigrationName, Migration>;

pub type MigrationSet = BTreeSet<MigrationName>;

#[derive(
    Deserialize, Serialize, Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Encode, Decode,
)]
pub enum MigrationName {
    #[n(0)]
    AccountCountData,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Migration {
    pub issue: Option<String>,
    pub block_height: u64,
}

pub fn run_migrations(
    current_height: u64,
    all_migrations: &MigrationMap,
    active_migrations: &mut MigrationSet,
    persistent_store: &mut merk::Merk,
) {
    let mut operations = vec![];
    for (migration_name, migration) in all_migrations {
        if current_height >= migration.block_height && active_migrations.insert(*migration_name) {
            operations.append(&mut migration_init(
                migration_name,
                active_migrations,
                persistent_store,
            ));
        }
    }
    operations.sort_by(|(a, _), (b, _)| a.cmp(b));
    persistent_store.apply(&operations).unwrap();
}

fn migration_init(
    name: &MigrationName,
    active_migrations: &MigrationSet,
    persistent_store: &merk::Merk,
) -> Vec<(Vec<u8>, Op)> {
    let mut operations = vec![];
    operations.push((
        MIGRATIONS_KEY.to_vec(),
        Op::Put(minicbor::to_vec(active_migrations).expect("Could not encode migrations to cbor")),
    ));
    match name {
        MigrationName::AccountCountData => {
            operations.append(&mut initial_metrics_data(persistent_store))
        }
    }
    operations
}
