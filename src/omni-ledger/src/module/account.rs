use minicbor::decode;
use omni::message::{RequestMessage, ResponseMessage};
use omni::protocol::Attribute;
use omni::server::module::OmniModuleInfo;
use omni::{Identity, OmniError, OmniModule};
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};

mod balance;
mod burn;
mod info;
mod mint;
mod send;

pub use balance::*;
pub use burn::*;
pub use info::*;
pub use mint::*;
pub use send::*;

/***
omni_module! {
    name       = LedgerModule,
    namespace  = "ledger",
    attributes = [2],

    fn info(&self, sender: &Identity, args: InfoArgs) -> Result<InfoReturns, OmniError>;
}
***/

pub const LEDGER_ATTRIBUTE: Attribute = Attribute::id(2);

lazy_static::lazy_static!(
    pub static ref LEDGER_MODULE_INFO: OmniModuleInfo = OmniModuleInfo {
        name: "LedgerModule".to_string(),
        attributes: vec![LEDGER_ATTRIBUTE],
        endpoints: vec![
            "ledger.info".to_string(),
            "ledger.balance".to_string(),
            "ledger.mint".to_string(),
            "ledger.burn".to_string(),
            "ledger.send".to_string(),
        ]
    };
);

pub trait LedgerModuleBackend: Send {
    fn info(&self, sender: &Identity, args: InfoArgs) -> Result<InfoReturns, OmniError>;
    fn balance(&self, sender: &Identity, args: BalanceArgs) -> Result<BalanceReturns, OmniError>;
    fn mint(&mut self, sender: &Identity, args: MintArgs) -> Result<(), OmniError>;
    fn burn(&mut self, sender: &Identity, args: BurnArgs) -> Result<(), OmniError>;
    fn send(&mut self, sender: &Identity, args: SendArgs) -> Result<(), OmniError>;
}

#[derive(Clone)]
pub struct LedgerModule<T>
where
    T: LedgerModuleBackend,
{
    backend: Arc<Mutex<T>>,
}

impl<T> Debug for LedgerModule<T>
where
    T: LedgerModuleBackend,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LedgerModule").finish()
    }
}

impl<T> LedgerModule<T>
where
    T: LedgerModuleBackend,
{
    pub fn new(backend: Arc<Mutex<T>>) -> Self {
        Self { backend }
    }
}

#[async_trait::async_trait]
impl<T> OmniModule for LedgerModule<T>
where
    T: LedgerModuleBackend,
{
    fn info(&self) -> &OmniModuleInfo {
        &LEDGER_MODULE_INFO
    }

    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        match message.method.as_str() {
            "ledger.info" => {
                decode::<'_, InfoArgs>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
            }
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
            "ledger.info" => encode(backend.info(&from, decode(&message.data)?)),
            "ledger.balance" => encode(backend.balance(&from, decode(&message.data)?)),
            "ledger.mint" => encode(backend.mint(&from, decode(&message.data)?)),
            "ledger.burn" => encode(backend.burn(&from, decode(&message.data)?)),
            "ledger.send" => encode(backend.send(&from, decode(&message.data)?)),
            _ => Err(OmniError::internal_server_error()),
        }?;

        Ok(ResponseMessage::from_request(
            &message,
            &message.to,
            Ok(result),
        ))
    }
}
