use crate::balance::{BalanceArgs, SymbolList};
use crate::burn::BurnArgs;
use crate::error::unauthorized;
use crate::info::InfoReturns;
use crate::mint::MintArgs;
use crate::send::SendArgs;
use crate::{error, LedgerStorage, TokenAmount};
use async_trait::async_trait;
use minicbor::{decode, Encoder};
use omni::message::{RequestMessage, ResponseMessage};
use omni::protocol::Attribute;
use omni::server::module::OmniModuleInfo;
use omni::{Identity, OmniError, OmniModule};
use omni_abci::module::{AbciInfo, AbciInit, OmniAbciModuleBackend};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

pub mod balance;
pub mod burn;
pub mod info;
pub mod mint;
pub mod send;

#[derive(serde::Deserialize, Debug, Default)]
pub struct InitialState {
    initial: BTreeMap<Identity, BTreeMap<String, u64>>,
    symbols: BTreeSet<String>,
    minters: Option<BTreeMap<String, BTreeSet<Identity>>>,
    hash: Option<String>,
}

/// A simple ledger that keeps transactions in memory.
#[derive(Default, Debug, Clone)]
pub struct LedgerModule {
    minters: BTreeMap<String, BTreeSet<Identity>>,
    symbols: BTreeSet<String>,
    storage: Arc<Mutex<LedgerStorage>>,
}

impl LedgerModule {
    pub fn new(initial_state: InitialState) -> Result<Self, OmniError> {
        let mut symbols = BTreeSet::new();
        let mut storage: LedgerStorage = Default::default();

        for symbol in initial_state.symbols {
            symbols.insert(symbol);
        }

        for (id, v) in &initial_state.initial {
            for (symbol, amount) in v {
                if symbols.contains(symbol) {
                    storage
                        .accounts
                        .entry(id.clone())
                        .or_default()
                        .insert(symbol.to_owned(), (*amount).into());
                } else {
                    return Err(error::unknown_symbol(symbol.to_owned()));
                }
            }
        }

        if let Some(h) = initial_state.hash {
            // Verify the hash.
            let actual = hex::encode(storage.hash());
            if actual != h {
                return Err(error::invalid_initial_state(h, actual));
            }
        }

        let mut minters: BTreeMap<String, BTreeSet<Identity>> = BTreeMap::new();
        if let Some(minter_list) = initial_state.minters {
            for (symbol, id_list) in minter_list {
                for id in id_list {
                    minters.entry(symbol.clone()).or_default().insert(id);
                }
            }
        }

        Ok(Self {
            minters,
            symbols,
            storage: Arc::new(Mutex::new(storage)),
        })
    }

    /// Checks whether an identity is the owner or not. Anonymous identities are forbidden.
    pub fn is_minter(&self, identity: &Identity, symbol: &str) -> bool {
        if identity.is_anonymous() {
            return false;
        }

        if let Some(minters) = self.minters.get(symbol) {
            minters.contains(identity)
        } else {
            false
        }
    }

    pub fn is_known_symbol(&self, symbol: &str) -> Result<(), OmniError> {
        if self.symbols.contains(symbol) {
            Ok(())
        } else {
            Err(error::unknown_symbol(symbol.to_string()))
        }
    }

    fn info(&self, _payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let mut bytes = Vec::with_capacity(512);
        let mut e = Encoder::new(&mut bytes);
        let storage = self.storage.lock().unwrap();

        // Hash the storage.
        let hash = storage.hash();
        let symbols: Vec<&str> = self.symbols.iter().map(|x| x.as_str()).collect();

        e.encode(InfoReturns {
            symbols: symbols.as_slice(),
            hash: hash.as_slice(),
        })
        .map_err(|e| OmniError::serialization_error(e.to_string()))?;

        Ok(bytes)
    }

    fn balance(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let BalanceArgs { account, symbols } =
            decode(payload).map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let identity = account.as_ref().unwrap_or(from);

        let storage = self.storage.lock().unwrap();
        let amounts = storage.get_multiple_balances(
            identity,
            symbols.unwrap_or_else(|| SymbolList(BTreeSet::new())).0,
        );
        minicbor::to_vec(amounts).map_err(|e| OmniError::serialization_error(e.to_string()))
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
        storage.send(from, &to, symbol, amount.clone())?;
        minicbor::to_vec(()).map_err(|e| OmniError::serialization_error(e.to_string()))
    }

    fn mint(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let MintArgs {
            account,
            amount,
            symbol,
        } = decode(payload).map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        if !self.is_minter(from, symbol) {
            return Err(error::unauthorized());
        }

        let mut storage = self.storage.lock().unwrap();
        storage.mint(&account, symbol, amount)?;

        minicbor::to_vec(()).map_err(|e| OmniError::serialization_error(e.to_string()))
    }

    fn burn(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let BurnArgs {
            account,
            amount,
            symbol,
        } = decode(payload).map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        if !self.is_minter(from, symbol) {
            return Err(error::unauthorized());
        }

        let mut storage = self.storage.lock().unwrap();
        storage.burn(&account, symbol, amount)?;

        minicbor::to_vec(()).map_err(|e| OmniError::serialization_error(e.to_string()))
    }
}

impl OmniAbciModuleBackend for LedgerModule {
    fn init(&self) -> AbciInit {
        AbciInit {
            endpoints: BTreeMap::from([
                ("ledger.info".to_string(), false),
                ("ledger.balance".to_string(), false),
                ("ledger.mint".to_string(), true),
                ("ledger.burn".to_string(), true),
                ("ledger.send".to_string(), true),
            ]),
        }
    }

    fn init_chain(&self) -> Result<(), OmniError> {
        Ok(())
    }

    fn info(&self) -> Result<AbciInfo, OmniError> {
        let storage = self.storage.lock().unwrap();
        Ok(AbciInfo {
            height: storage.height,
            hash: storage.hash(),
        })
    }

    fn commit(&self) -> Result<(), OmniError> {
        let mut storage = self.storage.lock().unwrap();
        Ok(storage.commit())
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
