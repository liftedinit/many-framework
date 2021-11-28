use crate::{error, LedgerStorage, TokenAmount};
use async_trait::async_trait;
use minicbor::encode::Write;
use minicbor::{decode, Decode, Decoder, Encode, Encoder};
use omni::message::{RequestMessage, ResponseMessage};
use omni::protocol::Attribute;
use omni::server::module::OmniModuleInfo;
use omni::{Identity, OmniError, OmniModule};
use omni_abci::module::{AbciInfo, AbciInit, OmniAbciModuleBackend};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

pub struct InfoArgs;
impl<'de> Decode<'de> for InfoArgs {
    fn decode(d: &mut Decoder<'de>) -> Result<Self, decode::Error> {
        Ok(Self)
    }
}

pub struct InfoReturns<'a> {
    pub height: u64,
    pub symbols: &'a [&'a str],
    pub hash: &'a [u8],
}
impl<'a> Encode for InfoReturns<'a> {
    fn encode<W: Write>(
        &self,
        e: &mut Encoder<W>,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        e.map(3)?
            .str("height")?
            .encode(self.height)?
            .str("symbols")?
            .encode(self.symbols)?
            .str("hash")?
            .encode(self.hash)?;

        Ok(())
    }
}

#[derive(serde::Deserialize, Debug, Default)]
pub struct InitialState {
    initial: BTreeMap<String, BTreeMap<String, u64>>,
    symbols: Vec<String>,
    hash: Option<String>,
}

/// A simple ledger that keeps transactions in memory.
#[derive(Default, Debug, Clone)]
pub struct LedgerModule {
    owner_id: Option<Identity>,
    symbols: BTreeSet<String>,
    storage: Arc<Mutex<LedgerStorage>>,
}

impl LedgerModule {
    pub fn new(
        owner_id: Option<Identity>,
        symbols: Vec<String>,
        initial_state: InitialState,
    ) -> Result<Self, OmniError> {
        let mut storage: LedgerStorage = Default::default();

        for (k, v) in &initial_state.initial {
            let id = Identity::from_str(k)?;
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

        Ok(Self {
            owner_id,
            symbols: BTreeSet::from_iter(symbols.iter().map(|x| x.to_string())),
            storage: Arc::new(Mutex::new(storage)),
        })
    }

    /// Checks whether an identity is the owner or not. Anonymous identities are forbidden.
    pub fn is_owner(&self, identity: &Identity) -> bool {
        if identity.is_anonymous() {
            return false;
        }

        match &self.owner_id {
            Some(o) => o == identity,
            None => false,
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
            height: storage.height,
            symbols: symbols.as_slice(),
            hash: hash.as_slice(),
        })
        .map_err(|e| OmniError::serialization_error(e.to_string()))?;

        Ok(bytes)
    }

    fn balance(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let mut d = minicbor::Decoder::new(payload);
        let (identity, symbol): (Option<Identity>, &str) = d
            .decode()
            .map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let storage = self.storage.lock().unwrap();
        if let Some(amount) = storage.get_balance(identity.as_ref().unwrap_or_else(|| from), symbol)
        {
            minicbor::to_vec(amount).map_err(|e| OmniError::serialization_error(e.to_string()))
        } else {
            minicbor::to_vec(TokenAmount::zero())
                .map_err(|e| OmniError::serialization_error(e.to_string()))
        }
    }

    fn send(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let mut d = minicbor::Decoder::new(payload);
        let (to, amount, symbol): (Identity, TokenAmount, &str) = d
            .decode()
            .map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let mut storage = self.storage.lock().unwrap();
        storage.send(&from, &to, symbol, amount.clone())?;

        if let Some(amount) = storage.get_balance(&from, symbol) {
            minicbor::to_vec(amount).map_err(|e| OmniError::serialization_error(e.to_string()))
        } else {
            minicbor::to_vec(TokenAmount::zero())
                .map_err(|e| OmniError::serialization_error(e.to_string()))
        }
    }

    fn mint(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        if !self.is_owner(from) {
            return Err(error::unauthorized());
        }

        let mut d = minicbor::Decoder::new(payload);
        let (to, amount, symbol): (Identity, TokenAmount, &str) = d
            .decode()
            .map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let mut storage = self.storage.lock().unwrap();
        storage.mint(&to, symbol, amount)?;

        if let Some(amount) = storage.get_balance(&to, symbol) {
            minicbor::to_vec(amount).map_err(|e| OmniError::serialization_error(e.to_string()))
        } else {
            minicbor::to_vec(TokenAmount::zero())
                .map_err(|e| OmniError::serialization_error(e.to_string()))
        }
    }

    fn burn(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        if !self.is_owner(from) {
            return Err(error::unauthorized());
        }

        let mut d = minicbor::Decoder::new(payload);
        let (to, amount, symbol): (Identity, TokenAmount, String) = d
            .decode()
            .map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let mut storage = self.storage.lock().unwrap();
        storage.burn(&to, &symbol, amount)?;

        Ok(Vec::new())
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
        let symbol = match message.method.as_str() {
            "ledger.info" => return Ok(()),
            "ledger.mint" => {
                decode::<'_, (Identity, TokenAmount, Option<&str>)>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?
                    .2
            }
            "ledger.burn" => {
                decode::<'_, (Identity, TokenAmount, Option<&str>)>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?
                    .2
            }
            "ledger.balance" => {
                decode::<'_, (Option<Identity>, Option<&str>)>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?
                    .1
            }
            "ledger.send" => {
                decode::<'_, (Identity, TokenAmount, Option<&str>)>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?
                    .2
            }
            _ => {
                return Err(OmniError::internal_server_error());
            }
        };
        symbol.map_or(Ok(()), |s| self.is_known_symbol(s))
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
