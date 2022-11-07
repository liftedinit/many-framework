use linkme::distributed_slice;
use many_error::ManyError;
use many_migration::{InnerMigration, Migration};
use std::collections::BTreeMap;

pub mod block_9400;
pub mod data;

pub type LedgerMigrations = BTreeMap<&'static str, Migration<'static, merk::Merk, ManyError>>;

// This is the global migration registry
// Doesn't contain any metadata
#[distributed_slice]
pub static MIGRATIONS: [InnerMigration<'static, merk::Merk, ManyError>] = [..];

pub const MIGRATIONS_KEY: &[u8] = b"/config/migrations";
