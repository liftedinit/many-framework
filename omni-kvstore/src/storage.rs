use crate::error;
use crate::utils::TokenAmount;
use fmerk::Op;
use omni::{Identity, OmniError};
use omni_abci::types::AbciCommitInfo;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::str::FromStr;

/// Returns the key for the persistent kv-store.
pub(crate) fn key_for(id: &Identity, symbol: &str) -> Vec<u8> {
    format!("/balances/{}/{}", id.to_string(), symbol).into_bytes()
}

pub type AclBTreeMap = BTreeMap<Vec<u8>, Vec<Identity>>;

pub struct KvStoreStorage {
    /// Simple ACL scheme. Any prefix that matches the key
    acls: AclBTreeMap,

    persistent_store: fmerk::Merk,

    /// When this is true, we do not commit every transactions as they come,
    /// but wait for a `commit` call before committing the batch to the
    /// persistent store.
    blockchain: bool,
}

impl std::fmt::Debug for KvStoreStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LedgerStorage").finish()
    }
}

impl KvStoreStorage {
    pub fn load<P: AsRef<Path>>(persistent_path: P, blockchain: bool) -> Result<Self, String> {
        let persistent_store = fmerk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

        let acls: AclBTreeMap =
            minicbor::decode(&persistent_store.get(b"/config/acls").unwrap().unwrap()).unwrap();

        Ok(Self {
            acls,
            persistent_store,
            blockchain,
        })
    }

    pub fn new<P: AsRef<Path>>(
        acls: AclBTreeMap,
        persistent_path: P,
        blockchain: bool,
    ) -> Result<Self, String> {
        let mut persistent_store = fmerk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

        let mut batch: Vec<fmerk::BatchEntry> = Vec::new();
        use itertools::Itertools;

        persistent_store
            .apply(&[(
                b"/config/acls".to_vec(),
                fmerk::Op::Put(minicbor::to_vec(&acls).unwrap()),
            )])
            .map_err(|e| e.to_string())?;
        persistent_store.commit(&[]).map_err(|e| e.to_string())?;

        Ok(Self {
            acls,
            persistent_store,
            blockchain,
        })
    }

    pub fn get_height(&self) -> u64 {
        self.persistent_store
            .get(b"/height")
            .unwrap()
            .map_or(0u64, |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                u64::from_be_bytes(bytes)
            })
    }

    pub fn commit(&mut self) -> AbciCommitInfo {
        let current_height = self.get_height() + 1;
        self.persistent_store
            .apply(&[(
                b"/height".to_vec(),
                fmerk::Op::Put(current_height.to_be_bytes().to_vec()),
            )])
            .unwrap();
        self.persistent_store.commit(&[]).unwrap();

        AbciCommitInfo {
            retain_height: current_height,
            hash: self.hash(),
        }
    }

    pub fn hash(&self) -> Vec<u8> {
        self.persistent_store.root_hash().to_vec()
    }

    pub fn get(&self, _id: &Identity, key: &[u8]) -> Result<Option<Vec<u8>>, OmniError> {
        self.persistent_store
            .get(&vec![b"/store/".to_vec(), key.to_vec()].concat())
            .map_err(|e| OmniError::unknown(e.to_string()))
    }

    pub fn put(&mut self, _id: &Identity, key: &[u8], value: Vec<u8>) -> Result<(), OmniError> {
        self.persistent_store
            .apply(&[(
                vec![b"/store/".to_vec(), key.to_vec()].concat(),
                Op::Put(value),
            )])
            .map_err(|e| OmniError::unknown(e.to_string()))
    }
}
