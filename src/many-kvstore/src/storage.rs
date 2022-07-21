use crate::error::unauthorized;
use many_error::ManyError;
use many_identity::Address;
use many_modules::abci_backend::AbciCommitInfo;
use merk::Op;
use std::collections::BTreeMap;
use std::path::Path;

pub type AclBTreeMap = BTreeMap<Vec<u8>, Vec<Address>>;

pub struct KvStoreStorage {
    /// Simple ACL scheme. Any prefix that matches the key
    acls: AclBTreeMap,

    persistent_store: merk::Merk,

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
        let persistent_store = merk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

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
        let mut persistent_store = merk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

        persistent_store
            .apply(&[(
                b"/config/acls".to_vec(),
                Op::Put(minicbor::to_vec(&acls).unwrap()),
            )])
            .map_err(|e| e.to_string())?;
        persistent_store.commit(&[]).map_err(|e| e.to_string())?;

        Ok(Self {
            acls,
            persistent_store,
            blockchain,
        })
    }

    pub fn height(&self) -> u64 {
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
        let current_height = self.height() + 1;
        self.persistent_store
            .apply(&[(
                b"/height".to_vec(),
                merk::Op::Put(current_height.to_be_bytes().to_vec()),
            )])
            .unwrap();
        self.persistent_store.commit(&[]).unwrap();

        AbciCommitInfo {
            retain_height: current_height,
            hash: self.hash().into(),
        }
    }

    pub fn hash(&self) -> Vec<u8> {
        self.persistent_store.root_hash().to_vec()
    }

    fn can_write(&self, id: &Address, key: &[u8]) -> bool {
        // TODO: remove this.
        if self.acls.is_empty() {
            return true;
        }

        let mut has_matched = false;

        // Any ACL that matches the key as a prefix in the ACL map and contains the identity
        // is a match.
        // Since BTreeMap is sorted by key, the first key that doesn't match after a key that
        // matches is out-of-bound and we won't match, so we can cut short.
        for (k, v) in self.acls.iter() {
            if key.starts_with(k) {
                has_matched = true;
                if v.contains(id) {
                    return true;
                }
            } else if has_matched {
                return false;
            }
        }

        false
    }

    #[allow(dead_code)]
    pub fn query(&self, prefix: Option<&[u8]>) -> Result<StorageIterator<'_>, ManyError> {
        if let Some(prefix) = prefix {
            Ok(StorageIterator::prefixed(&self.persistent_store, prefix))
        } else {
            Ok(StorageIterator::new(&self.persistent_store))
        }
    }

    pub fn get(&self, _id: &Address, key: &[u8]) -> Result<Option<Vec<u8>>, ManyError> {
        self.persistent_store
            .get(&vec![b"/store/".to_vec(), key.to_vec()].concat())
            .map_err(|e| ManyError::unknown(e.to_string()))
    }

    pub fn put(&mut self, id: &Address, key: &[u8], value: Vec<u8>) -> Result<(), ManyError> {
        if !self.can_write(id, key) {
            return Err(unauthorized());
        }

        self.persistent_store
            .apply(&[(
                vec![b"/store/".to_vec(), key.to_vec()].concat(),
                Op::Put(value),
            )])
            .map_err(|e| ManyError::unknown(e.to_string()))?;

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }
        Ok(())
    }

    pub fn delete(&mut self, id: &Address, key: &[u8]) -> Result<(), ManyError> {
        if !self.can_write(id, key) {
            return Err(unauthorized());
        }

        self.persistent_store
            .apply(&[(vec![b"/store/".to_vec(), key.to_vec()].concat(), Op::Delete)])
            .map_err(|e| ManyError::unknown(e.to_string()))?;

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }
        Ok(())
    }
}

pub struct StorageIterator<'a> {
    inner: merk::rocksdb::DBIterator<'a>,
}

#[allow(dead_code)]
impl<'a> StorageIterator<'a> {
    pub fn new(merk: &'a merk::Merk) -> Self {
        Self {
            inner: merk.iter_opt(merk::rocksdb::IteratorMode::Start, Default::default()),
        }
    }

    pub fn prefixed(merk: &'a merk::Merk, prefix: &[u8]) -> Self {
        if prefix.is_empty() {
            return Self::new(merk);
        }

        let mut upper_bound = prefix.to_vec();
        let last = upper_bound.last_mut().unwrap();
        *last += 1;

        let mut opts = merk::rocksdb::ReadOptions::default();
        opts.set_iterate_upper_bound(upper_bound);

        Self {
            inner: merk.iter_opt(
                merk::rocksdb::IteratorMode::From(prefix, merk::rocksdb::Direction::Forward),
                opts,
            ),
        }
    }
}

impl<'a> Iterator for StorageIterator<'a> {
    type Item = (Box<[u8]>, Box<[u8]>);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
