use async_trait::async_trait;
use minicbor::decode;
use omni::message::{RequestMessage, ResponseMessage};
use omni::protocol::Attribute;
use omni::server::module::OmniModuleInfo;
use omni::{Identity, OmniError, OmniModule};
use omni_abci::module::OmniAbciModuleBackend;
use omni_abci::types::{AbciCommitInfo, AbciInfo, AbciInit, EndpointInfo};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::info;

pub mod get;
pub mod info;
pub mod put;

use crate::storage::AclBTreeMap;
use crate::{error, storage::KvStoreStorage};
use get::{GetArgs, GetReturns};
use info::InfoReturns;
use put::{PutArgs, PutReturns};

/// The initial state schema, loaded from JSON.
#[derive(serde::Deserialize, Debug, Default)]
pub struct InitialStateJson {
    acls: AclBTreeMap,
    hash: Option<String>,
}

/// A simple kv-store.
#[derive(Debug, Clone)]
pub struct KvStoreModule {
    storage: Arc<Mutex<KvStoreStorage>>,
}

impl KvStoreModule {
    pub fn load<P: AsRef<Path>>(
        persistent_store_path: P,
        blockchain: bool,
    ) -> Result<Self, OmniError> {
        let storage = KvStoreStorage::load(persistent_store_path, blockchain)
            .map_err(|e| OmniError::unknown(e))?;
        Ok(Self {
            storage: Arc::new(Mutex::new(storage)),
        })
    }

    pub fn new<P: AsRef<Path>>(
        initial_state: InitialStateJson,
        persistence_store_path: P,
        blockchain: bool,
    ) -> Result<Self, OmniError> {
        let storage = KvStoreStorage::new(initial_state.acls, persistence_store_path, blockchain)
            .map_err(OmniError::unknown)?;

        if let Some(h) = initial_state.hash {
            // Verify the hash.
            let actual = hex::encode(storage.hash());
            if actual != h {
                return Err(error::invalid_initial_state(h, actual));
            }
        }

        info!(
            height = storage.height(),
            hash = hex::encode(storage.hash()).as_str()
        );

        Ok(Self {
            storage: Arc::new(Mutex::new(storage)),
        })
    }

    fn info(&self, _payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let storage = self.storage.lock().unwrap();

        // Hash the storage.
        let hash = storage.hash();

        minicbor::to_vec(InfoReturns { hash: hash.into() })
            .map_err(|e| OmniError::serialization_error(e.to_string()))
    }

    fn get(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let args: GetArgs = minicbor::decode(payload)
            .map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let storage = self.storage.lock().unwrap();

        let value = storage.get(from, &args.key)?;
        minicbor::to_vec(GetReturns { value })
            .map_err(|e| OmniError::serialization_error(e.to_string()))
    }

    fn put(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let args: PutArgs = minicbor::decode(payload)
            .map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let mut storage = self.storage.lock().unwrap();

        storage.put(from, &args.key, args.value)?;
        minicbor::to_vec(PutReturns {}).map_err(|e| OmniError::serialization_error(e.to_string()))
    }
}

// This module is always supported, but will only be added when created using an ABCI
// flag.
impl OmniAbciModuleBackend for KvStoreModule {
    #[rustfmt::skip]
    fn init(&self) -> AbciInit {
        AbciInit {
            endpoints: BTreeMap::from([
                ("kvstore.info".to_string(), EndpointInfo { should_commit: false }),
                ("kvstore.get".to_string(), EndpointInfo { should_commit: false }),
                ("kvstore.put".to_string(), EndpointInfo { should_commit: true }),
            ]),
        }
    }

    fn init_chain(&self) -> Result<(), OmniError> {
        Ok(())
    }

    fn info(&self) -> Result<AbciInfo, OmniError> {
        let storage = self.storage.lock().unwrap();
        Ok(AbciInfo {
            height: storage.height(),
            hash: storage.hash().into(),
        })
    }

    fn commit(&self) -> Result<AbciCommitInfo, OmniError> {
        let mut storage = self.storage.lock().unwrap();
        let info = storage.commit();
        Ok(info)
    }
}

const KVSTORE_ATTRIBUTE: Attribute = Attribute::id(3);

lazy_static::lazy_static!(
    pub static ref KVSTORE_MODULE_INFO: OmniModuleInfo = OmniModuleInfo {
        name: "KvStoreModule".to_string(),
        attributes: vec![KVSTORE_ATTRIBUTE],
        endpoints: vec![
            "kvstore.info".to_string(),
            "kvstore.get".to_string(),
            "kvstore.put".to_string(),
        ]
    };
);

#[async_trait]
impl OmniModule for KvStoreModule {
    fn info(&self) -> &OmniModuleInfo {
        &KVSTORE_MODULE_INFO
    }

    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        match message.method.as_str() {
            "kvstore.info" => return Ok(()),
            "kvstore.get" => {
                decode::<'_, GetArgs>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
            }
            "kvstore.put" => {
                decode::<'_, PutArgs>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
            }
            _ => {
                return Err(OmniError::internal_server_error());
            }
        };
        Ok(())
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let data = match message.method.as_str() {
            "kvstore.info" => self.info(&message.data),
            "kvstore.get" => self.get(&message.from.unwrap_or_default(), &message.data),
            "kvstore.put" => self.put(&message.from.unwrap_or_default(), &message.data),
            _ => Err(OmniError::internal_server_error()),
        }?;

        Ok(ResponseMessage::from_request(
            &message,
            &message.to,
            Ok(data),
        ))
    }
}
