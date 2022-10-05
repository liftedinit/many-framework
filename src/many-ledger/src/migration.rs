pub mod data;

use merk::Op;
use std::{collections::BTreeSet, fmt::Debug};

#[cfg(feature = "migrate_blocks")]
use many_protocol::ResponseMessage;

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

#[typetag::serde(tag = "type")]
pub trait Migration: Debug + Send + Sync {
    fn migrate(&self, persistent_store: &mut merk::Merk) -> Vec<(Vec<u8>, Op)>;
    fn block_height(&self) -> u64;
    fn issue(&self) -> Option<&str>;
    fn name(&self) -> &str;
}

impl PartialEq<Box<dyn Migration>> for Box<dyn Migration> {
    fn eq(&self, other: &Box<dyn Migration>) -> bool {
        self.name().eq(other.name())
    }
}

impl Eq for Box<dyn Migration> {}

impl PartialOrd<Box<dyn Migration>> for Box<dyn Migration> {
    fn partial_cmp(&self, other: &Box<dyn Migration>) -> Option<std::cmp::Ordering> {
        self.name().partial_cmp(other.name())
    }
}

impl Ord for Box<dyn Migration> {
    fn cmp(&self, other: &Box<dyn Migration>) -> std::cmp::Ordering {
        self.name().cmp(other.name())
    }
}

pub fn run_migrations(
    current_height: u64,
    all_migrations: &BTreeSet<Box<dyn Migration>>,
    active_migrations: &mut BTreeSet<String>,
    persistent_store: &mut merk::Merk,
) {
    let mut operations = vec![];
    for migration in all_migrations {
        if current_height >= migration.block_height()
            && active_migrations.insert(migration.name().to_string())
        {
            operations.push((
                MIGRATIONS_KEY.to_vec(),
                Op::Put(
                    minicbor::to_vec(active_migrations.clone())
                        .expect("Could not encode migrations to cbor"),
                ),
            ));
            operations.append(&mut migration.migrate(persistent_store))
        }
    }
    operations.sort_by(|(a, _), (b, _)| a.cmp(b));
    persistent_store.apply(&operations).unwrap();
}
