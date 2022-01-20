use crate::storage::AclBTreeMap;
use crate::{error, storage::KvStoreStorage};
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

mod get;
mod info;
mod put;

pub use get::*;
pub use info::*;
pub use put::*;

/// The initial state schema, loaded from JSON.
#[derive(serde::Deserialize, Debug, Default)]
pub struct InitialStateJson {
    acls: AclBTreeMap,
    hash: Option<String>,
}

/// A simple kv-store.
#[derive(Debug)]
pub struct KvStoreModuleImpl {
    storage: KvStoreStorage,
}

impl KvStoreModuleImpl {
    pub fn load<P: AsRef<Path>>(
        persistent_store_path: P,
        blockchain: bool,
    ) -> Result<Self, OmniError> {
        let storage = KvStoreStorage::load(persistent_store_path, blockchain)
            .map_err(|e| OmniError::unknown(e))?;

        Ok(Self { storage })
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

        Ok(Self { storage })
    }
}

// This module is always supported, but will only be added when created using an ABCI
// flag.
impl OmniAbciModuleBackend for KvStoreModuleImpl {
    #[rustfmt::skip]
    fn init(&mut self) -> AbciInit {
        AbciInit {
            endpoints: BTreeMap::from([
                ("kvstore.info".to_string(), EndpointInfo { should_commit: false }),
                ("kvstore.get".to_string(), EndpointInfo { should_commit: false }),
                ("kvstore.put".to_string(), EndpointInfo { should_commit: true }),
            ]),
        }
    }

    fn init_chain(&mut self) -> Result<(), OmniError> {
        Ok(())
    }

    fn info(&self) -> Result<AbciInfo, OmniError> {
        Ok(AbciInfo {
            height: self.storage.height(),
            hash: self.storage.hash().into(),
        })
    }

    fn commit(&mut self) -> Result<AbciCommitInfo, OmniError> {
        let info = self.storage.commit();
        Ok(info)
    }
}

impl KvStoreModuleBackend for KvStoreModuleImpl {
    fn info(&self, _sender: &Identity, _args: InfoArgs) -> Result<InfoReturns, OmniError> {
        // Hash the storage.
        let hash = self.storage.hash();

        Ok(InfoReturns { hash: hash.into() })
    }

    fn get(&self, sender: &Identity, args: GetArgs) -> Result<GetReturns, OmniError> {
        let value = self.storage.get(sender, &args.key)?;
        Ok(GetReturns { value })
    }

    fn put(&mut self, sender: &Identity, args: PutArgs) -> Result<PutReturns, OmniError> {
        self.storage.put(sender, &args.key, args.value)?;
        Ok(PutReturns {})
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

pub trait KvStoreModuleBackend: Send {
    fn info(&self, sender: &Identity, args: InfoArgs) -> Result<InfoReturns, OmniError>;
    fn get(&self, sender: &Identity, args: GetArgs) -> Result<GetReturns, OmniError>;
    fn put(&mut self, sender: &Identity, args: PutArgs) -> Result<PutReturns, OmniError>;
}

#[derive(Clone)]
pub struct KvStoreModule<T>
where
    T: KvStoreModuleBackend,
{
    backend: Arc<Mutex<T>>,
}

impl<T> std::fmt::Debug for KvStoreModule<T>
where
    T: KvStoreModuleBackend,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KvStoreModule").finish()
    }
}

impl<T> KvStoreModule<T>
where
    T: KvStoreModuleBackend,
{
    pub fn new(backend: Arc<Mutex<T>>) -> Self {
        Self { backend }
    }
}

#[async_trait::async_trait]
impl<T> OmniModule for KvStoreModule<T>
where
    T: KvStoreModuleBackend,
{
    fn info(&self) -> &OmniModuleInfo {
        &KVSTORE_MODULE_INFO
    }

    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        match message.method.as_str() {
            "kvstore.info" => {
                decode::<'_, InfoArgs>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
            }
            "kvstore.get" => {
                decode::<'_, GetArgs>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
            }
            "kvstore.put" => {
                decode::<'_, PutArgs>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
            }

            _ => return Err(OmniError::invalid_method_name(message.method.clone())),
        };
        Ok(())
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        fn decode<'a, T: minicbor::Decode<'a>>(data: &'a [u8]) -> Result<T, OmniError> {
            minicbor::decode(data).map_err(|e| OmniError::deserialization_error(e.to_string()))
        }
        fn encode<T: minicbor::Encode>(result: Result<T, OmniError>) -> Result<Vec<u8>, OmniError> {
            minicbor::to_vec(result?).map_err(|e| OmniError::serialization_error(e.to_string()))
        }

        let from = message.from.unwrap_or_default();
        let mut backend = self.backend.lock().unwrap();
        let result = match message.method.as_str() {
            "kvstore.info" => encode(backend.info(&from, decode(&message.data)?)),
            "kvstore.get" => encode(backend.get(&from, decode(&message.data)?)),
            "kvstore.put" => encode(backend.put(&from, decode(&message.data)?)),
            _ => Err(OmniError::internal_server_error()),
        }?;

        Ok(ResponseMessage::from_request(
            &message,
            &message.to,
            Ok(result),
        ))
    }
}
