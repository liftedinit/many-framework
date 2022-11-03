use linkme::distributed_slice;
use many_error::ManyError;
use many_migration::InnerMigration;

pub mod block_9400;
pub mod data;

// #[cfg(feature = "migration_testing")]
// pub mod dummy_hotfix;

// This is the global migration registry
// Doesn't contain any metadata
#[distributed_slice]
pub static MIGRATIONS: [InnerMigration<'static, merk::Merk, ManyError>] = [..];
pub const MIGRATIONS_KEY: &[u8] = b"/config/migrations";
