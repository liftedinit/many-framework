use omni::message::{RequestMessage, ResponseMessage};
use omni::protocol::Attribute;
use omni::server::module::OmniModuleInfo;
use omni::{Identity, OmniError, OmniModule};
use std::sync::{Arc, Mutex};

mod info;
mod list;

pub use info::*;
pub use list::*;

pub const LEDGER_TRANSACTIONS_ATTRIBUTE: Attribute = Attribute::id(4);

lazy_static::lazy_static!(
    pub static ref LEDGER_MODULE_INFO: OmniModuleInfo = OmniModuleInfo {
        name: "LedgerModule".to_string(),
        attributes: vec![LEDGER_TRANSACTIONS_ATTRIBUTE],
        endpoints: vec![
            "ledger.transactions".to_string(),
            "ledger.list".to_string(),
        ]
    };
);

pub trait LedgerTransactionsModuleBackend: Send {
    fn transactions(
        &self,
        sender: &Identity,
        args: TransactionsArgs,
    ) -> Result<TransactionsReturns, OmniError>;
    fn list(&mut self, sender: &Identity, args: ListArgs) -> Result<ListReturns, OmniError>;
}

#[derive(Clone)]
pub struct LedgerTransactionsModule<T>
where
    T: LedgerTransactionsModuleBackend,
{
    backend: Arc<Mutex<T>>,
}

impl<T> std::fmt::Debug for LedgerTransactionsModule<T>
where
    T: LedgerTransactionsModuleBackend,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LedgerTransactionsModule").finish()
    }
}

impl<T> LedgerTransactionsModule<T>
where
    T: LedgerTransactionsModuleBackend,
{
    pub fn new(backend: Arc<Mutex<T>>) -> Self {
        Self { backend }
    }
}

#[async_trait::async_trait]
impl<T> OmniModule for LedgerTransactionsModule<T>
where
    T: LedgerTransactionsModuleBackend,
{
    fn info(&self) -> &OmniModuleInfo {
        &LEDGER_MODULE_INFO
    }

    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        match message.method.as_str() {
            "ledger.transactions" => {
                minicbor::decode::<'_, TransactionsArgs>(&message.data)
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
            }
            "ledger.list" => {
                minicbor::decode::<'_, ListArgs>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
            }

            _ => return Err(OmniError::internal_server_error()),
        }
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
            "ledger.transactions" => encode(backend.transactions(&from, decode(&message.data)?)),
            "ledger.list" => encode(backend.list(&from, decode(&message.data)?)),
            _ => Err(OmniError::internal_server_error()),
        }?;

        Ok(ResponseMessage::from_request(
            &message,
            &message.to,
            Ok(result),
        ))
    }
}
