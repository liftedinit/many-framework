use many_modules::data::{DataInfo, DataValue};
use many_types::ledger::TokenAmount;
use merk::{
    rocksdb::{self, ReadOptions},
    Op,
};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(feature = "migrate_blocks")]
use many_protocol::ResponseMessage;
use serde::{Deserialize, Serialize};

use crate::{
    module::{ACCOUNT_TOTAL_COUNT_INDEX, NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX},
    storage::{DATA_ATTRIBUTES_KEY, DATA_INFO_KEY, MIGRATIONS_KEY},
};

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

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Migration {
    pub issue: Option<String>,
    pub block_height: u64,
}

pub fn run_migrations(
    current_height: u64,
    all_migrations: &BTreeMap<String, Migration>,
    active_migrations: &mut BTreeSet<String>,
    persistent_store: &mut merk::Merk,
) {
    let mut operations = vec![];
    for (migration_name, migration) in all_migrations {
        if current_height >= migration.block_height
            && active_migrations.insert(migration_name.clone())
        {
            operations.append(&mut migration_init(
                migration_name,
                active_migrations,
                persistent_store,
            ));
        }
    }
    operations.sort_by(|(a, _), (b, _)| a.cmp(b));
    persistent_store.apply(&operations).unwrap();
}

fn migration_init(
    name: &str,
    active_migrations: &BTreeSet<String>,
    persistent_store: &merk::Merk,
) -> Vec<(Vec<u8>, Op)> {
    let mut operations = vec![];
    operations.push((
        MIGRATIONS_KEY.to_vec(),
        Op::Put(minicbor::to_vec(active_migrations).expect("Could not encode migrations to cbor")),
    ));
    if name == "account_count_data" {
        operations.append(&mut initial_metrics_data(persistent_store));
    }
    operations
}

fn initial_metrics_data(persistent_store: &merk::Merk) -> Vec<(Vec<u8>, Op)> {
    let mut total_accounts: u64 = 0;
    let mut non_zero: u64 = 0;

    let mut upper_bound = b"/balances".to_vec();
    *upper_bound.last_mut().unwrap() += 1;
    let mut opts = ReadOptions::default();
    opts.set_iterate_upper_bound(upper_bound);

    let iterator = persistent_store.iter_opt(
        rocksdb::IteratorMode::From(b"/balances", rocksdb::Direction::Forward),
        opts,
    );
    for item in iterator {
        let (key, value) = item.expect("Error while reading the DB");
        let value = merk::tree::Tree::decode(key.to_vec(), value.as_ref());
        let amount = TokenAmount::from(value.value().to_vec());
        total_accounts += 1;
        if !amount.is_zero() {
            non_zero += 1
        }
    }

    let data = BTreeMap::from([
        (
            *ACCOUNT_TOTAL_COUNT_INDEX,
            DataValue::Counter(total_accounts),
        ),
        (
            *NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX,
            DataValue::Counter(non_zero),
        ),
    ]);
    let data_info = BTreeMap::from([
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
    ]);
    vec![
        (
            DATA_ATTRIBUTES_KEY.to_vec(),
            Op::Put(minicbor::to_vec(data).unwrap()),
        ),
        (
            DATA_INFO_KEY.to_vec(),
            Op::Put(minicbor::to_vec(data_info).unwrap()),
        ),
    ]
}
