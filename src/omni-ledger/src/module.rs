use async_trait::async_trait;
use minicbor::decode;
use omni::message::{RequestMessage, ResponseMessage};
use omni::protocol::Attribute;
use omni::server::module::OmniModuleInfo;
use omni::{Identity, OmniError, OmniModule};
use omni_abci::module::OmniAbciModuleBackend;
use omni_abci::types::{AbciCommitInfo, AbciInfo, AbciInit, EndpointInfo};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::info;

pub mod balance;
pub mod burn;
pub mod info;
pub mod mint;
pub mod send;

use crate::module::balance::BalanceReturns;
use crate::{error, storage::LedgerStorage};
use balance::{BalanceArgs, SymbolList};
use burn::BurnArgs;
use error::unauthorized;
use info::InfoReturns;
use mint::MintArgs;
use send::SendArgs;

/// The initial state schema, loaded from JSON.
#[derive(serde::Deserialize, Debug, Default)]
pub struct InitialStateJson {
    initial: BTreeMap<Identity, BTreeMap<String, u128>>,
    symbols: BTreeSet<String>,
    minters: Option<BTreeMap<String, Vec<Identity>>>,
    hash: Option<String>,
}

/// A simple ledger that keeps transactions in memory.
#[derive(Debug, Clone)]
pub struct LedgerModule {
    storage: Arc<Mutex<LedgerStorage>>,
}

impl LedgerModule {
    pub fn new<P: AsRef<Path>>(
        initial_state: Option<InitialStateJson>,
        persistence_store_path: P,
        blockchain: bool,
    ) -> Result<Self, OmniError> {
        let storage = if let Some(state) = initial_state {
            let storage = LedgerStorage::new(
                state.symbols,
                state.initial,
                state.minters.unwrap_or_default(),
                persistence_store_path,
                blockchain,
            )
            .map_err(OmniError::unknown)?;

            if let Some(h) = state.hash {
                // Verify the hash.
                let actual = hex::encode(storage.hash());
                if actual != h {
                    return Err(error::invalid_initial_state(h, actual));
                }
            }

            storage
        } else {
            LedgerStorage::load(persistence_store_path, blockchain).unwrap()
        };

        info!(
            height = storage.get_height(),
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
        let symbols: Vec<&str> = storage.get_symbols();

        info!(
            "info(): hash={} symbols={:?}",
            hex::encode(storage.hash()).as_str(),
            symbols
        );

        minicbor::to_vec(InfoReturns {
            symbols: symbols.iter().map(|x| x.to_string()).collect(),
            hash: hash.into(),
        })
        .map_err(|e| OmniError::serialization_error(e.to_string()))
    }

    fn balance(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let BalanceArgs {
            account,
            symbols,
            proof,
        } = decode(payload).map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let identity = account.as_ref().unwrap_or(from);

        let mut storage = self.storage.lock().unwrap();
        let symbols = symbols.unwrap_or_else(|| SymbolList(BTreeSet::new())).0;

        let balances = storage.get_multiple_balances(identity, &symbols);
        info!("balance({}, {:?}): {:?}", identity, &symbols, &balances);
        let returns = if proof.unwrap_or(false) {
            BalanceReturns {
                balances: None,
                proof: Some(storage.generate_proof(identity, &symbols)?.into()),
                hash: storage.hash().into(),
            }
        } else {
            BalanceReturns {
                balances: Some(
                    balances
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), v))
                        .collect(),
                ),
                proof: None,
                hash: storage.hash().into(),
            }
        };

        minicbor::to_vec(returns).map_err(|e| OmniError::serialization_error(e.to_string()))
    }

    fn send(&self, sender: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let SendArgs {
            from,
            to,
            amount,
            symbol,
        } = decode(payload).map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let from = from.as_ref().unwrap_or(sender);

        // TODO: allow some ACLs or delegation on the ledger.
        if from != sender {
            return Err(unauthorized());
        }

        let mut storage = self.storage.lock().unwrap();
        storage.send(from, &to, &symbol.to_string(), amount)?;
        minicbor::to_vec(()).map_err(|e| OmniError::serialization_error(e.to_string()))
    }

    fn mint(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let MintArgs {
            account,
            amount,
            symbol,
        } = decode(payload).map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let mut storage = self.storage.lock().unwrap();

        if storage.can_mint(from, symbol) {
            storage.mint(&account, &symbol.to_string(), amount)?;
        } else {
            return Err(unauthorized());
        }

        minicbor::to_vec(()).map_err(|e| OmniError::serialization_error(e.to_string()))
    }

    fn burn(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let BurnArgs {
            account,
            amount,
            symbol,
        } = decode(payload).map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let mut storage = self.storage.lock().unwrap();
        if storage.can_mint(from, symbol) {
            storage.burn(&account, &symbol.to_string(), amount)?;
        } else {
            return Err(unauthorized());
        }

        minicbor::to_vec(()).map_err(|e| OmniError::serialization_error(e.to_string()))
    }
}

// This module is always supported, but will only be added when created using an ABCI
// flag.
impl OmniAbciModuleBackend for LedgerModule {
    #[rustfmt::skip]
    fn init(&self) -> AbciInit {
        AbciInit {
            endpoints: BTreeMap::from([
                ("ledger.info".to_string(), EndpointInfo { should_commit: false }),
                ("ledger.balance".to_string(), EndpointInfo { should_commit: false }),
                ("ledger.mint".to_string(), EndpointInfo { should_commit: true }),
                ("ledger.burn".to_string(), EndpointInfo { should_commit: true }),
                ("ledger.send".to_string(), EndpointInfo { should_commit: true }),
            ]),
        }
    }

    fn init_chain(&self) -> Result<(), OmniError> {
        info!("abci.init_chain()",);
        Ok(())
    }

    fn info(&self) -> Result<AbciInfo, OmniError> {
        let storage = self.storage.lock().unwrap();

        info!(
            "abci.info(): height={} hash={}",
            storage.get_height(),
            hex::encode(storage.hash()).as_str()
        );
        Ok(AbciInfo {
            height: storage.get_height(),
            hash: storage.hash().into(),
        })
    }

    fn commit(&self) -> Result<AbciCommitInfo, OmniError> {
        let mut storage = self.storage.lock().unwrap();
        let result = storage.commit();

        info!(
            "abci.commit(): retain_height={} hash={}",
            result.retain_height,
            hex::encode(storage.hash()).as_str()
        );
        Ok(result)
    }
}

const LEDGER_ATTRIBUTE: Attribute = Attribute::new(
    2,
    &[
        "ledger.info",
        "ledger.balance",
        "ledger.mint",
        "ledger.burn",
        "ledger.send",
    ],
);

lazy_static::lazy_static!(
    pub static ref LEDGER_MODULE_INFO: OmniModuleInfo = OmniModuleInfo {
        name: "LedgerModule".to_string(),
        attributes: vec![LEDGER_ATTRIBUTE],
    };
);

#[async_trait]
impl OmniModule for LedgerModule {
    fn info(&self) -> &OmniModuleInfo {
        &LEDGER_MODULE_INFO
    }

    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        match message.method.as_str() {
            "ledger.info" => return Ok(()),
            "ledger.mint" => {
                decode::<'_, MintArgs>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
            }
            "ledger.burn" => {
                decode::<'_, BurnArgs>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
            }
            "ledger.balance" => {
                decode::<'_, BalanceArgs>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
            }
            "ledger.send" => {
                decode::<'_, SendArgs>(message.data.as_slice())
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
            "ledger.info" => self.info(&message.data),
            "ledger.balance" => self.balance(&message.from.unwrap_or_default(), &message.data),
            "ledger.mint" => self.mint(&message.from.unwrap_or_default(), &message.data),
            "ledger.burn" => self.burn(&message.from.unwrap_or_default(), &message.data),
            "ledger.send" => self.send(&message.from.unwrap_or_default(), &message.data),
            _ => Err(OmniError::internal_server_error()),
        }?;

        Ok(ResponseMessage::from_request(
            &message,
            &message.to,
            Ok(data),
        ))
    }
}
