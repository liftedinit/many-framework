use std::collections::BTreeMap;
use std::fmt::Debug;

use many_modules::data::{DataIndex, DataInfo, DataValue};
use many_types::ledger::TokenAmount;
use merk::rocksdb::{self, ReadOptions};
use merk::Op;
use serde::{Deserialize, Serialize};

use crate::storage::{DATA_ATTRIBUTES_KEY, DATA_INFO_KEY};

use super::Migration;

lazy_static::lazy_static!(
    pub static ref ACCOUNT_TOTAL_COUNT_INDEX: DataIndex =
        DataIndex::new(0).with_index(2).with_index(0);

    pub static ref NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX: DataIndex =
        DataIndex::new(0).with_index(2).with_index(1);
);

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountCountData {
    block_height: u64,
    issue: Option<String>,
}

#[typetag::serde]
impl Migration for AccountCountData {
    fn block_height(&self) -> u64 {
        self.block_height
    }

    fn issue(&self) -> Option<&str> {
        self.issue.as_deref()
    }

    fn name(&self) -> &str {
        "AccountCountData"
    }

    fn migrate(&self, persistent_store: &mut merk::Merk) -> Vec<(Vec<u8>, Op)> {
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
}
