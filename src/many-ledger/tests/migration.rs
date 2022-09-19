pub mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::*;
use many_identity::testing::identity;
use many_ledger::data_migration::Migration;

#[test]
fn migration() {
    let mut harness = Setup::new(true);
    let migration = Migration {
        issue: None,
        block_height: 2,
    };
    harness.module_impl = harness.module_impl.with_migrations(BTreeMap::from([(
        "account_count_data".to_string(),
        migration,
    )]));
    harness.set_balance(harness.id, 1_000_000, *MFX_SYMBOL);

    let (_height, _a1) = harness.block(|h| {
        h.send_(h.id, identity(2), 500_000u32);
        h.create_account_(AccountType::Multisig)
    });

    let (height, _a2) = harness.block(|h| {
        h.send_(h.id, identity(3), 500_000u32);
        h.create_account_(AccountType::Multisig)
    });

    assert_eq!(height, 2);
}
