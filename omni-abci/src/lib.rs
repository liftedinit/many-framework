use async_trait::async_trait;
use omni::message::{RequestMessage, ResponseMessage};
use omni::protocol::Attribute;
use omni::server::module::{OmniModule, OmniModuleInfo};
use omni::{Identity, OmniError};
use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};
use std::iter::FromIterator;
use std::sync::{Arc, Mutex};
use tendermint_proto::abci::{RequestCheckTx, ResponseCheckTx};

pub mod abci_app;
pub mod omni_app;

pub const ABCI_SERVER: Attribute = Attribute {
    id: 1000,
    endpoints: &["abci."],
};

/// A module that specifies that this network is the backend of an ABCI blockchain.
/// This adds keys to the status for whether certain functions are query or commands,
/// since those need to be sent separately through ABCI.
#[derive(Debug, Clone)]
pub struct AbciApplicationModule {
    queries: Vec<String>,
}

impl AbciApplicationModule {
    pub fn new() -> Self {}
}

/// A module that implements an ABCI interface which allows to query, broadcast and expose
/// other ABCI endpoints. This specific module does also implement the blockchain attribute.
#[derive(Clone)]
pub struct AbciModule {
    client: Arc<Mutex<tendermint_rpc::WebSocketClient>>,
    identity: Identity,
}

impl Debug for AbciModule {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AbciModule")
            .field("client", &"...")
            .field("identity", &self.identity)
            .finish()
    }
}

impl AbciModule {
    pub fn new(client: Arc<Mutex<tendermint_rpc::WebSocketClient>>, identity: Identity) -> Self {
        Self { client, identity }
    }
}

lazy_static::lazy_static!(
    pub static ref ABCI_MODULE_INFO: OmniModuleInfo = OmniModuleInfo {
        name: "AbciModule".to_string(),
        attributes: vec![ABCI_SERVER],
    };
);

#[async_trait]
impl OmniModule for AbciModule {
    #[inline]
    fn info(&self) -> &OmniModuleInfo {
        &ABCI_MODULE_INFO
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let mut client = self.client.lock().unwrap();

        match message.method.as_str() {
            // "abci.check_tx" => {
            //     let response = client.check_tx(RequestCheckTx {
            //         tx: message.data.clone(),
            //         r#type: 0,
            //     });
            //
            //     match response {
            //         Ok(ResponseCheckTx { code, data, .. }) if code == 0 => Ok(
            //             ResponseMessage::from_request(&message, &self.identity, Ok(data)),
            //         ),
            //         Ok(ResponseCheckTx { code, data, .. }) => Err(OmniError::application_specific(
            //             1000 * 10000 + code,
            //             "ABCI Response Error: {msg}".to_string(),
            //             BTreeMap::from_iter(vec![("msg".to_string(), hex::encode(&data))]),
            //         )),
            //         Err(_) => Err(OmniError::internal_server_error()),
            //     }
            // }
            _ => Err(OmniError::internal_server_error()),
        }
    }
}
