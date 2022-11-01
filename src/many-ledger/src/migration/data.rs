use crate::migration::MIGRATIONS;
use crate::storage::data::{DATA_ATTRIBUTES_KEY, DATA_INFO_KEY};
use linkme::distributed_slice;
use many_error::ManyError;
use many_migration::InnerMigration;
use many_modules::data::{DataIndex, DataInfo, DataValue};
use many_types::ledger::TokenAmount;
use merk::rocksdb::ReadOptions;
use merk::{rocksdb, Op};
use std::collections::BTreeMap;

lazy_static::lazy_static!(
    pub static ref ACCOUNT_TOTAL_COUNT_INDEX: DataIndex =
        DataIndex::new(0).with_index(2).with_index(0);

    pub static ref NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX: DataIndex =
        DataIndex::new(0).with_index(2).with_index(1);
);

fn get_data_from_db(storage: &merk::Merk) -> (u64, u64) {
    let mut num_unique_accounts: u64 = 0;
    let mut num_non_zero_account: u64 = 0;

    let mut upper_bound = b"/balances".to_vec();
    *upper_bound.last_mut().unwrap() += 1;
    let mut opts = ReadOptions::default();
    opts.set_iterate_upper_bound(upper_bound);

    let iterator = storage.iter_opt(
        rocksdb::IteratorMode::From(b"/balances", rocksdb::Direction::Forward),
        opts,
    );
    for item in iterator {
        let (key, value) = item.expect("Error while reading the DB");
        let value = merk::tree::Tree::decode(key.to_vec(), value.as_ref());
        let amount = TokenAmount::from(value.value().to_vec());
        num_unique_accounts += 1;
        if !amount.is_zero() {
            num_non_zero_account += 1
        }
    }

    (num_unique_accounts, num_non_zero_account)
}

fn data_info() -> BTreeMap<DataIndex, DataInfo> {
    BTreeMap::from([
        (
            *ACCOUNT_TOTAL_COUNT_INDEX,
            DataInfo {
                r#type: many_modules::data::DataType::Counter,
                shortname: "accountTotalCount".to_string(),
            },
        ),
        (
            *NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX,
            DataInfo {
                r#type: many_modules::data::DataType::Counter,
                shortname: "nonZeroAccountTotalCount".to_string(),
            },
        ),
    ])
}

fn data_value(
    num_unique_accounts: u64,
    num_non_zero_account: u64,
) -> BTreeMap<DataIndex, DataValue> {
    BTreeMap::from([
        (
            *ACCOUNT_TOTAL_COUNT_INDEX,
            DataValue::Counter(num_unique_accounts),
        ),
        (
            *NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX,
            DataValue::Counter(num_non_zero_account),
        ),
    ])
}

fn initialize(storage: &mut merk::Merk) -> Result<(), ManyError> {
    let (num_unique_accounts, num_non_zero_account) = get_data_from_db(storage);

    storage
        .apply(&[
            (
                DATA_ATTRIBUTES_KEY.to_vec(),
                Op::Put(
                    minicbor::to_vec(data_value(num_unique_accounts, num_non_zero_account))
                        .unwrap(),
                ),
            ),
            (
                DATA_INFO_KEY.to_vec(),
                Op::Put(minicbor::to_vec(data_info()).unwrap()),
            ),
        ])
        .map_err(ManyError::unknown)?; // TODO: Custom error
    Ok(())
}

// TODO: Update based on Tx?
fn update(storage: &mut merk::Merk) -> Result<(), ManyError> {
    let (num_unique_accounts, num_non_zero_account) = get_data_from_db(storage);

    storage
        .apply(&[(
            DATA_ATTRIBUTES_KEY.to_vec(),
            Op::Put(
                minicbor::to_vec(data_value(num_unique_accounts, num_non_zero_account)).unwrap(),
            ),
        )])
        .map_err(ManyError::unknown)?; // TODO: Custom error

    Ok(())
}

#[distributed_slice(MIGRATIONS)]
static ACCOUNT_COUNT_DATA_ATTRIBUTE: InnerMigration<merk::Merk, ManyError> =
    InnerMigration::new_initialize_update(
        &initialize,
        &update,
        "Account Count Data Attribute",
        r#"
            Provides the total number of unique addresses. 
            Provides the total number of unique addresses with a non-zero balance.
            "#,
    );
