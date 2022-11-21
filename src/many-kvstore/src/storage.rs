use crate::module::{KvStoreMetadata, KvStoreMetadataWrapper};
use many_error::ManyError;
use many_identity::Address;
use many_modules::abci_backend::AbciCommitInfo;
use many_modules::events::EventInfo;
use many_types::{Either, Timestamp};
use merk::{BatchEntry, Op};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

mod account;
mod event;

use crate::error;
use event::EventId;

const KVSTORE_ROOT: &[u8] = b"s";
const KVSTORE_ACL_ROOT: &[u8] = b"a";

// Left-shift the height by this amount of bits
const HEIGHT_EVENTID_SHIFT: u64 = 32;

#[derive(Serialize, Deserialize, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct Key {
    #[serde(with = "hex::serde")]
    key: Vec<u8>,
}

pub type AclMap = BTreeMap<Key, KvStoreMetadataWrapper>;

pub struct KvStoreStorage {
    persistent_store: merk::Merk,

    /// When this is true, we do not commit every transactions as they come,
    /// but wait for a `commit` call before committing the batch to the
    /// persistent store.
    blockchain: bool,

    latest_event_id: EventId,
    current_time: Option<Timestamp>,
    current_hash: Option<Vec<u8>>,
    next_account_id: u32,
    account_identity: Address,
}

impl std::fmt::Debug for KvStoreStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KvStoreStorage").finish()
    }
}

impl KvStoreStorage {
    #[inline]
    pub fn set_time(&mut self, time: Timestamp) {
        self.current_time = Some(time);
    }
    #[inline]
    pub fn now(&self) -> Timestamp {
        self.current_time.unwrap_or_else(Timestamp::now)
    }

    pub fn load<P: AsRef<Path>>(persistent_path: P, blockchain: bool) -> Result<Self, String> {
        let persistent_store = merk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

        let next_account_id = persistent_store
            .get(b"/config/account_id")
            .unwrap()
            .map_or(0, |x| {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(x.as_slice());
                u32::from_be_bytes(bytes)
            });

        let account_identity: Address = Address::from_bytes(
            &persistent_store
                .get(b"/config/identity")
                .expect("Could not open storage.")
                .expect("Could not find key '/config/identity' in storage."),
        )
        .map_err(|e| e.to_string())?;

        let height = persistent_store.get(b"/height").unwrap().map_or(0u64, |x| {
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(x.as_slice());
            u64::from_be_bytes(bytes)
        });

        // The call to `saturating_sub()` is required to fix
        // https://github.com/liftedinit/many-framework/issues/289
        //
        // The `commit()` function computes the `latest_event_id` using the previous height while
        // the following line computes the `latest_event_id` using the current height.
        //
        // The discrepancy will lead to an application hash mismatch if the block following the `load()` contains
        // a transaction.
        let latest_event_id = EventId::from(height.saturating_sub(1) << HEIGHT_EVENTID_SHIFT);

        Ok(Self {
            persistent_store,
            blockchain,
            current_time: None,
            current_hash: None,
            latest_event_id,
            next_account_id,
            account_identity,
        })
    }

    pub fn new<P: AsRef<Path>>(
        acl: AclMap,
        identity: Address,
        persistent_path: P,
        blockchain: bool,
    ) -> Result<Self, String> {
        let mut persistent_store = merk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

        let mut batch: Vec<BatchEntry> = Vec::new();

        batch.push((b"/config/identity".to_vec(), Op::Put(identity.to_vec())));

        // Initialize DB with ACL
        for (k, v) in acl.into_iter() {
            batch.push((
                vec![KVSTORE_ACL_ROOT.to_vec(), k.key.to_vec()].concat(),
                Op::Put(minicbor::to_vec(v).map_err(|e| e.to_string())?),
            ));
        }

        persistent_store
            .apply(batch.as_slice())
            .map_err(|e| e.to_string())?;

        persistent_store.commit(&[]).map_err(|e| e.to_string())?;

        Ok(Self {
            persistent_store,
            blockchain,
            current_time: None,
            current_hash: None,
            latest_event_id: EventId::from(vec![0]),
            next_account_id: 0,
            account_identity: identity,
        })
    }

    fn inc_height(&mut self) -> u64 {
        let current_height = self.get_height();
        self.persistent_store
            .apply(&[(
                b"/height".to_vec(),
                Op::Put((current_height + 1).to_be_bytes().to_vec()),
            )])
            .unwrap();
        current_height
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
        let height = self.inc_height();
        let retain_height = 0;
        self.persistent_store.commit(&[]).unwrap();

        let hash = self.persistent_store.root_hash().to_vec();
        self.current_hash = Some(hash.clone());

        self.latest_event_id = EventId::from(height << HEIGHT_EVENTID_SHIFT);

        AbciCommitInfo {
            retain_height,
            hash: hash.into(),
        }
    }

    pub fn hash(&self) -> Vec<u8> {
        self.current_hash
            .as_ref()
            .map_or_else(|| self.persistent_store.root_hash().to_vec(), |x| x.clone())
    }

    fn _get(&self, key: &[u8], prefix: &[u8]) -> Result<Option<Vec<u8>>, ManyError> {
        self.persistent_store
            .get(&vec![prefix.to_vec(), key.to_vec()].concat())
            .map_err(|e| ManyError::unknown(e.to_string()))
    }

    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, ManyError> {
        if let Some(cbor) = self._get(key, KVSTORE_ACL_ROOT)? {
            let meta: KvStoreMetadata = minicbor::decode(&cbor)
                .map_err(|e| ManyError::deserialization_error(e.to_string()))?;

            if let Some(either) = meta.disabled {
                match either {
                    Either::Left(false) => {}
                    _ => return Err(error::key_disabled()),
                }
            }
        }
        self._get(key, KVSTORE_ROOT)
    }

    pub fn get_metadata(&self, key: &[u8]) -> Result<Option<Vec<u8>>, ManyError> {
        self._get(key, KVSTORE_ACL_ROOT)
    }

    pub fn put(
        &mut self,
        meta: &KvStoreMetadata,
        key: &[u8],
        value: Vec<u8>,
    ) -> Result<(), ManyError> {
        self.persistent_store
            .apply(&[
                (
                    vec![KVSTORE_ACL_ROOT.to_vec(), key.to_vec()].concat(),
                    Op::Put(
                        minicbor::to_vec(meta)
                            .map_err(|e| ManyError::serialization_error(e.to_string()))?,
                    ),
                ),
                (
                    vec![KVSTORE_ROOT.to_vec(), key.to_vec()].concat(),
                    Op::Put(value.clone()),
                ),
            ])
            .map_err(|e| ManyError::unknown(e.to_string()))?;

        self.log_event(EventInfo::KvStorePut {
            key: key.to_vec().into(),
            value: value.into(),
            owner: meta.owner,
        });

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }
        Ok(())
    }

    pub fn disable(&mut self, meta: &KvStoreMetadata, key: &[u8]) -> Result<(), ManyError> {
        self.persistent_store
            .apply(&[(
                vec![KVSTORE_ACL_ROOT.to_vec(), key.to_vec()].concat(),
                Op::Put(
                    minicbor::to_vec(meta)
                        .map_err(|e| ManyError::serialization_error(e.to_string()))?,
                ),
            )])
            .map_err(|e| ManyError::unknown(e.to_string()))?;

        let reason = if let Some(disabled) = &meta.disabled {
            match disabled {
                Either::Right(reason) => Some(reason),
                _ => None,
            }
        } else {
            None
        };

        self.log_event(EventInfo::KvStoreDisable {
            key: key.to_vec().into(),
            owner: meta.owner,
            reason: reason.cloned(),
        });

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }
        Ok(())
    }
}
