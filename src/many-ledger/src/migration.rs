pub mod data;

use merk::Op;
use minicbor::{Decode, Encode};
use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};
use strum::EnumString;

#[cfg(feature = "migrate_blocks")]
use many_protocol::ResponseMessage;
use serde::{de::Visitor, Deserialize, Serialize};

use crate::storage::MIGRATIONS_KEY;

#[cfg(feature = "block_9400")]
mod block_9400;

lazy_static::lazy_static!(
    pub static ref MIGRATION_RUNNERS: BTreeMap<MigrationName, MigrationRunner> =
        BTreeMap::from([(MigrationName::AccountCountData, to_runner(data::migrate))]);
);

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

pub type MigrationRunner = Box<dyn Fn(&merk::Merk) -> Vec<(Vec<u8>, Op)> + Send + Sync>;

/// Used to populate the MIGRATION_RUNNERS variable with normal functions
fn to_runner(
    f: impl Fn(&merk::Merk) -> Vec<(Vec<u8>, Op)> + Send + Sync + 'static,
) -> MigrationRunner {
    Box::new(f) as MigrationRunner
}

/// The name of a migration, which will be referenced in the migration
/// configuration TOML file. Every new migration is a new variant in
/// this enum.
#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Encode, Decode, EnumString)]
pub enum MigrationName {
    /// AccountCountData is a migration which introduces data
    /// attributes and metrics for known accounts and non-empty
    /// accounts.
    #[n(0)]
    AccountCountData,
}

// MigrationName has custom Serialize and Deserialize because the
// derived one neither produces nor consumes TOML strings, and TOML
// keys are always necessarily TOML strings.
impl Serialize for MigrationName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{:?}", self))
    }
}

struct MigrationNameVisitor;

impl<'de> Visitor<'de> for MigrationNameVisitor {
    type Value = MigrationName;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("A variant of the enum MigrationName")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        MigrationName::from_str(v)
            .map_err(|_| E::invalid_type(serde::de::Unexpected::Str(v), &"MigrationName"))
    }
}

impl<'de> Deserialize<'de> for MigrationName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(MigrationNameVisitor)
    }
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
            operations.push((
                MIGRATIONS_KEY.to_vec(),
                Op::Put(
                    minicbor::to_vec(active_migrations.clone())
                        .expect("Could not encode migrations to cbor"),
                ),
            ));
            operations.append(&mut MIGRATION_RUNNERS[migration_name](persistent_store));
        }
    }
    operations.sort_by(|(a, _), (b, _)| a.cmp(b));
    persistent_store.apply(&operations).unwrap();
}
