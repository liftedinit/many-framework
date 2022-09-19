pub mod common;

use std::collections::BTreeMap;

use common::*;
use many_identity::testing::identity;
use many_ledger::{
    data_migration::Migration,
    module::{ACCOUNT_TOTAL_COUNT_INDEX, NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX},
};
use many_modules::{
    data::{DataGetInfoArgs, DataModuleBackend, DataQueryArgs},
    EmptyArg,
};
use many_types::{ledger::TokenAmount, VecOrSingle};
use num_bigint::BigInt;

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

    assert_eq!(
        harness
            .module_impl
            .info(&harness.id, EmptyArg)
            .unwrap()
            .indices
            .len(),
        0
    );

    let (height, _a2) = harness.block(|h| {
        h.send_(h.id, identity(3), 500_000u32);
        h.create_account_(AccountType::Multisig)
    });

    assert_eq!(height, 2);
    assert_eq!(
        harness
            .module_impl
            .info(&harness.id, EmptyArg)
            .unwrap()
            .indices
            .len(),
        2
    );
    assert_eq!(
        harness
            .module_impl
            .get_info(
                &harness.id,
                DataGetInfoArgs {
                    indices: VecOrSingle(vec![
                        *ACCOUNT_TOTAL_COUNT_INDEX,
                        *NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX
                    ])
                }
            )
            .unwrap()
            .len(),
        2
    );
    let query = harness
        .module_impl
        .query(
            &harness.id,
            DataQueryArgs {
                indices: VecOrSingle(vec![
                    *ACCOUNT_TOTAL_COUNT_INDEX,
                    *NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX,
                ]),
            },
        )
        .unwrap();
    let total: BigInt = query[&*ACCOUNT_TOTAL_COUNT_INDEX]
        .clone()
        .try_into()
        .unwrap();
    let non_zero: BigInt = query[&*NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX]
        .clone()
        .try_into()
        .unwrap();

    assert_eq!(total, BigInt::from(4));
    assert_eq!(non_zero, BigInt::from(4));
    assert_eq!(
        harness.balance(harness.id, *MFX_SYMBOL).unwrap(),
        TokenAmount::zero()
    );
}
