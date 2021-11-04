use async_trait::async_trait;
use minicose::{CoseKey, CoseSign1, Ed25519CoseKeyBuilder};
use omni::message::{RequestMessage, ResponseMessage};
use omni::transport::LowLevelOmniRequestHandler;
use omni::{Identity, OmniError};
use ring::signature::{Ed25519KeyPair, KeyPair};
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};
use tendermint_abci::Client as AbciClient;
use tendermint_proto::abci::{RequestDeliverTx, RequestQuery};

pub enum AbciMessageType {
    Query,
    Command,
}

pub trait OmniAbciFrontend: Send + Sync + Debug {
    fn message_type(&self, message: &RequestMessage) -> AbciMessageType;
    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError>;
}

pub struct AbciHttpServer {
    client: Arc<Mutex<AbciClient>>,
    identity: Identity,
    keypair: Option<Ed25519KeyPair>,
    frontend: Box<dyn OmniAbciFrontend>,
}

impl Debug for AbciHttpServer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AbciHttpServer")
            .field("client", &"...")
            .field("identity", &self.identity)
            .field("keypair", &"...")
            .field("frontend", &self.frontend)
            .finish()
    }
}

impl AbciHttpServer {
    pub fn new<F: OmniAbciFrontend + 'static>(
        client: Arc<Mutex<AbciClient>>,
        frontend: F,
        identity: Identity,
        keypair: Option<Ed25519KeyPair>,
    ) -> Self {
        let cose_key: Option<CoseKey> = keypair.as_ref().map(|kp| {
            let x = kp.public_key().as_ref().to_vec();
            Ed25519CoseKeyBuilder::default()
                .x(x)
                .kid(identity.to_vec())
                .build()
                .unwrap()
                .into()
        });

        assert!(
            identity.matches_key(&cose_key),
            "Identity does not match keypair."
        );
        assert!(identity.is_addressable(), "Identity is not addressable.");

        Self {
            client,
            identity,
            keypair,
            frontend: Box::new(frontend),
        }
    }

    fn execute_inner(&self, envelope: CoseSign1) -> Result<ResponseMessage, OmniError> {
        let message = omni::message::decode_request_from_cose_sign1(envelope.clone())
            .and_then(|request| self.frontend.validate(&request).map(|_| request))?;

        let mut client = self.client.lock().unwrap();

        match self.frontend.message_type(&message) {
            AbciMessageType::Query => {
                let response = client
                    .query(RequestQuery {
                        data: envelope
                            .to_bytes()
                            .map_err(|_| OmniError::internal_server_error())?,
                        path: "".to_string(),
                        height: 0,
                        prove: false,
                    })
                    .map_err(|_| OmniError::internal_server_error())?;

                Ok(ResponseMessage::from_request(
                    &message,
                    &self.identity,
                    Ok(response.value),
                ))
            }
            AbciMessageType::Command => {
                let response = client
                    .deliver_tx(RequestDeliverTx {
                        tx: envelope
                            .to_bytes()
                            .map_err(|_| OmniError::internal_server_error())?,
                    })
                    .map_err(|_| OmniError::internal_server_error())?;
                eprintln!("command... {:?}", response);

                Ok(ResponseMessage::from_request(
                    &message,
                    &self.identity,
                    Ok(response.data),
                ))
            }
        }
    }
}

#[async_trait]
impl LowLevelOmniRequestHandler for AbciHttpServer {
    async fn execute(&self, envelope: CoseSign1) -> Result<CoseSign1, String> {
        let response = self
            .execute_inner(envelope)
            .unwrap_or_else(|err| ResponseMessage::error(&self.identity, err));

        omni::message::encode_cose_sign1_from_response(
            response,
            self.identity.clone(),
            &self.keypair,
        )
    }
}
