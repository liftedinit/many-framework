pub mod common;

use common::*;
use many_identity::testing::identity;
use many_ledger::{
    migration::MigrationMap,
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
    let migrations_str = r#"
    [AccountCountData]
    block_height = 2
    "#;
    let migrations: MigrationMap = toml::from_str(migrations_str).unwrap();
    harness.module_impl = harness.module_impl.with_migrations(migrations);
    harness.set_balance(harness.id, 1_000_000, *MFX_SYMBOL);

    let (_height, a1) = harness.block(|h| {
        h.send_(h.id, identity(2), 250_000u32);
        identity(2)
    });

    let balance = harness.balance(a1, *MFX_SYMBOL).unwrap();

    assert_eq!(balance, 250_000u32);

    assert_eq!(
        harness
            .module_impl
            .info(&harness.id, EmptyArg)
            .unwrap()
            .indices
            .len(),
        0
    );

    let (_height, a2) = harness.block(|h| {
        h.send_(h.id, identity(3), 250_000u32);
        identity(3)
    });

    let balance = harness.balance(a2, *MFX_SYMBOL).unwrap();

    assert_eq!(balance, 250_000u32);

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
        TokenAmount::from(500_000u64),
    );

    let (_height, a3) = harness.block(|h| {
        h.send_(h.id, identity(4), 500_000u32);
        identity(4)
    });

    let balance = harness.balance(a3, *MFX_SYMBOL).unwrap();

    assert_eq!(balance, 500_000u32);

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

    assert_eq!(total, BigInt::from(5));
    assert_eq!(non_zero, BigInt::from(4));
    assert_eq!(
        harness.balance(harness.id, *MFX_SYMBOL).unwrap(),
        TokenAmount::zero()
    );
}