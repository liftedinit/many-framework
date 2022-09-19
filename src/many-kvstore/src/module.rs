use crate::{
    error,
    storage::{AclMap, KvStoreStorage},
};
use many_error::{ManyError, Reason};
use many_identity::Address;
use many_modules::abci_backend::{
    AbciBlock, AbciCommitInfo, AbciInfo, AbciInit, BeginBlockReturn, EndpointInfo, InitChainReturn,
    ManyAbciModuleBackend,
};
use many_modules::account::Role;
use many_modules::kvstore::{
    DisableArgs, DisableReturn, GetArgs, GetReturns, InfoArg, InfoReturns,
    KvStoreCommandsModuleBackend, KvStoreModuleBackend, PutArgs, PutReturn, QueryArgs,
    QueryReturns,
};
use many_types::{Either, Timestamp};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::path::Path;
use tracing::info;

pub mod account;
mod event;

// The initial state schema, loaded from JSON.
#[derive(serde::Deserialize, Debug, Default)]
pub struct InitialStateJson {
    acl: AclMap,
    identity: Address,
    hash: Option<String>,
}

/// A simple kv-store.
#[derive(Debug)]
pub struct KvStoreModuleImpl {
    storage: KvStoreStorage,
}

/// The KvStoreMetadata mimics the QueryReturns structure but adds serde capabilities
#[derive(Clone, Debug, minicbor::Encode, minicbor::Decode, serde::Deserialize)]
#[serde(remote = "QueryReturns")]
#[cbor(map)]
pub struct KvStoreMetadata {
    #[n(0)]
    pub owner: Address,

    #[n(1)]
    #[serde(skip_deserializing)]
    pub disabled: Option<Either<bool, Reason<u64>>>,
}

#[derive(Debug, serde::Deserialize, minicbor::Encode, minicbor::Decode)]
#[serde(transparent)]
#[cbor(transparent)]
pub struct KvStoreMetadataWrapper(
    #[serde(with = "KvStoreMetadata")]
    #[n(0)]
    QueryReturns,
);

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
        let storage = KvStoreStorage::new(
            initial_state.acl,
            initial_state.identity,
            persistence_store_path,
            blockchain,
        )
        .map_err(ManyError::unknown)?;

        if let Some(h) = initial_state.hash {
            // Verify the hash.
            let actual = hex::encode(storage.hash());
            if actual != h {
                return Err(error::invalid_initial_hash(h, actual));
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
                ("kvstore.disable".to_string(), EndpointInfo { is_command: true }),

                // Accounts
                ("account.create".to_string(), EndpointInfo { is_command: true }),
                ("account.setDescription".to_string(), EndpointInfo { is_command: true }),
                ("account.listRoles".to_string(), EndpointInfo { is_command: false }),
                ("account.getRoles".to_string(), EndpointInfo { is_command: false }),
                ("account.addRoles".to_string(), EndpointInfo { is_command: true }),
                ("account.removeRoles".to_string(), EndpointInfo { is_command: true }),
                ("account.info".to_string(), EndpointInfo { is_command: false }),
                ("account.disable".to_string(), EndpointInfo { is_command: true }),
                ("account.addFeatures".to_string(), EndpointInfo { is_command: true }),

                // Events
                ("events.info".to_string(), EndpointInfo { is_command: false }),
                ("events.list".to_string(), EndpointInfo { is_command: false }),
            ]),
        })
    }

    fn init_chain(&mut self) -> Result<InitChainReturn, ManyError> {
        Ok(InitChainReturn {})
    }

    fn begin_block(&mut self, info: AbciBlock) -> Result<BeginBlockReturn, ManyError> {
        let time = info.time;
        info!(
            "abci.block_begin(): time={:?} curr_height={}",
            time,
            self.storage.get_height()
        );

        if let Some(time) = time {
            let time = Timestamp::new(time)?;
            self.storage.set_time(time);
        }

        Ok(BeginBlockReturn {})
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
    fn info(&self, _sender: &Address, _args: InfoArg) -> Result<InfoReturns, ManyError> {
        // Hash the storage.
        let hash = self.storage.hash();

        Ok(InfoReturns { hash: hash.into() })
    }

    fn get(&self, _sender: &Address, args: GetArgs) -> Result<GetReturns, ManyError> {
        let value = self.storage.get(&args.key)?;
        Ok(GetReturns {
            value: value.map(|x| x.into()),
        })
    }

    fn query(&self, _sender: &Address, args: QueryArgs) -> Result<QueryReturns, ManyError> {
        // TODO: Custom error type
        Ok(minicbor::decode(
            &self
                .storage
                .get_metadata(&args.key)?
                .ok_or(ManyError::unknown("TODO: Fix me"))?,
        )
        .map_err(|e| ManyError::deserialization_error(e.to_string()))?)
    }
}

impl KvStoreCommandsModuleBackend for KvStoreModuleImpl {
    fn put(&mut self, sender: &Address, args: PutArgs) -> Result<PutReturn, ManyError> {
        let key: Vec<u8> = args.key.into();
        let owner = if let Some(ref alternative_owner) = args.alternative_owner {
            self.validate_alternative_owner(
                sender,
                alternative_owner,
                [Role::CanKvStoreWrite, Role::Owner],
            )?;
            alternative_owner
        } else {
            sender
        };

        self.can_write(owner, key.clone())?;

        let meta = KvStoreMetadata {
            owner: *owner,
            disabled: Some(Either::Left(false)),
        };
        self.storage.put(&meta, &key, args.value.into())?;
        Ok(PutReturn {})
    }

    fn disable(&mut self, sender: &Address, args: DisableArgs) -> Result<DisableReturn, ManyError> {
        let key: Vec<u8> = args.key.into();
        let owner = if let Some(ref alternative_owner) = args.alternative_owner {
            self.validate_alternative_owner(
                sender,
                alternative_owner,
                [Role::CanKvStoreDisable, Role::Owner],
            )?;
            alternative_owner
        } else {
            sender
        };

        self.can_disable(owner, key.clone())?;

        let meta = KvStoreMetadata {
            owner: *owner,
            disabled: Some(Either::Left(true)),
        };

        self.storage.disable(&meta, &key)?;
        Ok(DisableReturn {})
    }
}
