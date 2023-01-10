use {
    crate::migration::MIGRATIONS, linkme::distributed_slice, many_error::ManyError,
    many_migration::InnerMigration,
};

fn initialize(_: &mut merk::Merk) -> Result<(), ManyError> {
    Ok(())
}

#[distributed_slice(MIGRATIONS)]
pub static NOMIC_MIGRATION: InnerMigration<merk::Merk, ManyError> = InnerMigration::new_initialize(
    initialize,
    "Merk Migration",
    "Update to the current head of the nomic fork of the Merk library",
);
