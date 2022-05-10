use crate::storage::AclBTreeMap;
use crate::{error, storage::KvStoreStorage};
use many::server::module::abci_backend::{
    AbciCommitInfo, AbciInfo, AbciInit, EndpointInfo, ManyAbciModuleBackend,
};
use many::server::module::kvstore::{
    DeleteArgs, DeleteReturn, GetArgs, GetReturns, InfoArg, InfoReturns,
    KvStoreCommandsModuleBackend, KvStoreModuleBackend, PutArgs, PutReturns,
};
use many::{Identity, ManyError};
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
    ) -> Result<Self, ManyError> {
        let storage =
            KvStoreStorage::load(persistent_store_path, blockchain).map_err(ManyError::unknown)?;

        Ok(Self { storage })
    }

    pub fn new<P: AsRef<Path>>(
        initial_state: InitialStateJson,
        persistence_store_path: P,
        blockchain: bool,
    ) -> Result<Self, ManyError> {
        let storage = KvStoreStorage::new(initial_state.acls, persistence_store_path, blockchain)
            .map_err(ManyError::unknown)?;

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
impl ManyAbciModuleBackend for KvStoreModuleImpl {
    #[rustfmt::skip]
    fn init(&mut self) -> Result<AbciInit, ManyError> {
        Ok(AbciInit {
            endpoints: BTreeMap::from([
                ("kvstore.info".to_string(), EndpointInfo { is_command: false }),
                ("kvstore.get".to_string(), EndpointInfo { is_command: false }),
                ("kvstore.put".to_string(), EndpointInfo { is_command: true }),
            ]),
        })
    }

    fn init_chain(&mut self) -> Result<(), ManyError> {
        Ok(())
    }

    fn info(&self) -> Result<AbciInfo, ManyError> {
        Ok(AbciInfo {
            height: self.storage.height(),
            hash: self.storage.hash().into(),
        })
    }

    fn commit(&mut self) -> Result<AbciCommitInfo, ManyError> {
        let info = self.storage.commit();
        Ok(info)
    }
}

impl KvStoreModuleBackend for KvStoreModuleImpl {
    fn info(&self, _sender: &Identity, _args: InfoArg) -> Result<InfoReturns, ManyError> {
        // Hash the storage.
        let hash = self.storage.hash();

        Ok(InfoReturns { hash: hash.into() })
    }

    fn get(&self, sender: &Identity, args: GetArgs) -> Result<GetReturns, ManyError> {
        let value = self.storage.get(sender, &args.key)?;
        Ok(GetReturns {
            value: value.map(|x| x.into()),
        })
    }
}

impl KvStoreCommandsModuleBackend for KvStoreModuleImpl {
    fn put(&mut self, sender: &Identity, args: PutArgs) -> Result<PutReturns, ManyError> {
        self.storage.put(sender, &args.key, args.value.into())?;
        Ok(PutReturns {})
    }

    fn delete(&mut self, sender: &Identity, args: DeleteArgs) -> Result<DeleteReturn, ManyError> {
        self.storage.delete(sender, &args.key)?;
        Ok(DeleteReturn {})
    }
}
