use async_trait::async_trait;
use minicose::CoseSign1;
use omni::message::{RequestMessage, ResponseMessage};
use omni::protocol::Attribute;
use omni::server::function::FunctionMapRequestHandler;
use omni::server::module::{OmniModule, OmniModuleInfo};
use omni::transport::OmniRequestHandler;
use omni::{Identity, OmniError};
use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};
use std::iter::FromIterator;
use std::net::ToSocketAddrs;
use std::sync::{Arc, Mutex};
use tendermint_abci::Client as AbciClient;
use tendermint_proto::abci::{RequestCheckTx, RequestEcho, ResponseCheckTx};

pub mod application;

pub const ABCI_SERVER: Attribute = Attribute {
    id: 1000,
    endpoints: &["abci.check_tx"],
};

#[derive(Clone)]
pub struct AbciModule {
    client: Arc<Mutex<AbciClient>>,
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
    pub fn new(client: Arc<Mutex<AbciClient>>, identity: Identity) -> Self {
        Self { client, identity }
    }
}

#[async_trait]
impl OmniModule for AbciModule {
    #[inline]
    fn info(&self) -> OmniModuleInfo {
        OmniModuleInfo {
            name: "AbciModule".to_string(),
            attributes: vec![ABCI_SERVER],
        }
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let tx = message.data.clone();
        let mut client = self.client.lock().unwrap();

        match message.method.as_str() {
            "abci.check_tx" => {
                let response = client.check_tx(RequestCheckTx {
                    tx: message.data.clone(),
                    r#type: 0,
                });

                match response {
                    Ok(ResponseCheckTx { code, data, .. }) if code == 0 => Ok(
                        ResponseMessage::from_request(&message, &self.identity, Ok(data)),
                    ),
                    Ok(ResponseCheckTx { code, data, .. }) => Err(OmniError::application_specific(
                        1000 * 10000 + code,
                        "ABCI Response Error: {msg}".to_string(),
                        BTreeMap::from_iter(vec![("msg".to_string(), hex::encode(&data))]),
                    )),
                    Err(_) => Err(OmniError::internal_server_error()),
                }
            }

            _ => Err(OmniError::internal_server_error()),
        }
    }
}
