use crate::storage::AclBTreeMap;
use crate::{error, storage::KvStoreStorage};
use omni::server::module::abci_backend::{
    AbciCommitInfo, AbciInfo, AbciInit, EndpointInfo, OmniAbciModuleBackend,
};
use omni::server::module::kvstore::{
    GetArgs, GetReturns, InfoArgs, InfoReturns, KvStoreModuleBackend, PutArgs, PutReturns,
};
use omni::{Identity, OmniError};
use std::collections::BTreeMap;
use std::path::Path;
use tracing::info;

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
    fn init(&mut self) -> Result<AbciInit, OmniError> {
        Ok(AbciInit {
            endpoints: BTreeMap::from([
                ("kvstore.info".to_string(), EndpointInfo { should_commit: false }),
                ("kvstore.get".to_string(), EndpointInfo { should_commit: false }),
                ("kvstore.put".to_string(), EndpointInfo { should_commit: true }),
            ]),
        })
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
        Ok(GetReturns {
            value: value.map(|x| x.into()),
        })
    }

    fn put(&mut self, sender: &Identity, args: PutArgs) -> Result<PutReturns, OmniError> {
        self.storage.put(sender, &args.key, args.value.into())?;
        Ok(PutReturns {})
    }
}
