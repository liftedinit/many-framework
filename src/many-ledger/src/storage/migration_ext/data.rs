use std::collections::BTreeMap;

use many_identity::Address;
use many_modules::data::{DataIndex, DataValue};
use many_types::ledger::TokenAmount;
use merk::Op;

use crate::{
    migration::data::{ACCOUNT_TOTAL_COUNT_INDEX, NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX},
    storage::{key_for_account_balance, LedgerStorage, DATA_ATTRIBUTES_KEY},
};

pub trait DataExt {
    fn update_data_attributes(
        &mut self,
        from: &Address,
        to: &Address,
        amount: TokenAmount,
        symbol: &Address,
    );

    fn data_attributes(&self) -> Option<BTreeMap<DataIndex, DataValue>>;
}

impl DataExt for LedgerStorage {
    fn data_attributes(&self) -> Option<BTreeMap<DataIndex, DataValue>> {
        self.persistent_store
            .get(DATA_ATTRIBUTES_KEY)
            .expect("Error while reading the DB")
            .map(|x| minicbor::decode(&x).unwrap())
    }

    fn update_data_attributes(
        &mut self,
        from: &Address,
        to: &Address,
        amount: TokenAmount,
        symbol: &Address,
    ) {
        if let Some(mut attributes) = self.data_attributes() {
            let key_to = key_for_account_balance(to, symbol);
            if self
                .persistent_store
                .get(&key_to)
                .expect("Error communicating with the DB")
                .is_none()
            {
                attributes
                    .entry(*ACCOUNT_TOTAL_COUNT_INDEX)
                    .and_modify(|x| {
                        if let DataValue::Counter(count) = x {
                            *count += 1;
                        }
                    });
                attributes
                    .entry(*NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX)
                    .and_modify(|x| {
                        if let DataValue::Counter(count) = x {
                            *count += 1;
                        }
                    });
            }
            let balance_from = self.get_balance(from, symbol);
            if balance_from == amount {
                attributes
                    .entry(*NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX)
                    .and_modify(|x| {
                        if let DataValue::Counter(count) = x {
                            *count -= 1;
                        }
                    });
            }
            self.persistent_store
                .apply(&[(
                    DATA_ATTRIBUTES_KEY.to_vec(),
                    Op::Put(minicbor::to_vec(attributes).unwrap()),
                )])
                .unwrap();
        }
    }
}
