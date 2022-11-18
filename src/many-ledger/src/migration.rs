use linkme::distributed_slice;
use many_error::ManyError;
use many_migration::{InnerMigration, Migration, MigrationSet};
use std::collections::BTreeMap;

pub mod block_9400;
pub mod data;
pub mod memo;

#[cfg(feature = "migration_testing")]
pub mod dummy_hotfix;

pub type LedgerMigrations = MigrationSet<'static, merk::Merk>;

// This is the global migration registry
// Doesn't contain any metadata
#[distributed_slice]
pub static MIGRATIONS: [InnerMigration<merk::Merk, ManyError>] = [..];
