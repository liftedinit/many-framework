use crate::storage::LedgerStorage;
use many_modules::data::{DataIndex, DataInfo, DataValue};
use std::collections::BTreeMap;

pub const DATA_ATTRIBUTES_KEY: &[u8] = b"/data/attributes";
pub const DATA_INFO_KEY: &[u8] = b"/data/info";

impl LedgerStorage {
    pub(crate) fn data_info(&self) -> Option<BTreeMap<DataIndex, DataInfo>> {
        self.persistent_store
            .get(DATA_INFO_KEY)
            .expect("Error while reading the DB")
            .map(|x| minicbor::decode(&x).unwrap())
    }

    pub(crate) fn data_attributes(&self) -> Option<BTreeMap<DataIndex, DataValue>> {
        self.persistent_store
            .get(DATA_ATTRIBUTES_KEY)
            .expect("Error while reading the DB")
            .map(|x| minicbor::decode(&x).unwrap())
    }
}
