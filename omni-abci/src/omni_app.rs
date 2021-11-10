use async_trait::async_trait;
use minicose::{CoseKey, CoseSign1, Ed25519CoseKeyBuilder};
use omni::message::{RequestMessage, ResponseMessage};
use omni::transport::LowLevelOmniRequestHandler;
use omni::{Identity, OmniError};
use ring::signature::{Ed25519KeyPair, KeyPair};
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use tendermint_proto::abci::{RequestDeliverTx, RequestQuery};
use tendermint_rpc::{Client, WebSocketClient};

pub enum AbciMessageType {
    Query,
    Command,
}

pub trait OmniAbciFrontend: Send + Sync + Debug {
    fn message_type(&self, message: &RequestMessage) -> AbciMessageType;
    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError>;
}

pub struct AbciHttpServer {
    client: WebSocketClient,
    identity: Identity,
    keypair: Option<Ed25519KeyPair>,
}

impl Debug for AbciHttpServer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AbciHttpServer")
            .field("client", &"...")
            .field("identity", &self.identity)
            .field("keypair", &"...")
            .finish()
    }
}

impl AbciHttpServer {
    pub fn new(
        client: tendermint_rpc::WebSocketClient,
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
        }
    }

    async fn execute_inner(&self, envelope: CoseSign1) -> Result<ResponseMessage, OmniError> {
        let message = omni::message::decode_request_from_cose_sign1(envelope.clone())?;
        let client = self.client.clone();

        // match self.frontend.message_type(&message) {
        //     AbciMessageType::Query => {
        //         let response = async move {
        //             let bytes = envelope.to_bytes().unwrap();
        //             client
        //                 .abci_query(None, bytes, None, false)
        //                 .await
        //                 .map_err(|_| OmniError::internal_server_error())
        //         }
        //         .await?;
        //
        //         Ok(ResponseMessage::from_request(
        //             &message,
        //             &self.identity,
        //             Ok(response.value),
        //         ))
        //     }
        //     AbciMessageType::Command => {
        let response = client
            .broadcast_tx_async(tendermint::abci::Transaction::from(
                envelope
                    .to_bytes()
                    .map_err(|_| OmniError::internal_server_error())?,
            ))
            .await
            .map_err(|_| OmniError::internal_server_error())?;

        Ok(ResponseMessage::from_request(
            &message,
            &self.identity,
            Ok(response.data.value().to_vec()),
        ))
        //     }
        // }
    }
}

#[async_trait]
impl LowLevelOmniRequestHandler for AbciHttpServer {
    async fn execute(&self, envelope: CoseSign1) -> Result<CoseSign1, String> {
        let response = self
            .execute_inner(envelope)
            .await
            .unwrap_or_else(|err| ResponseMessage::error(&self.identity, err));

        omni::message::encode_cose_sign1_from_response(
            response,
            self.identity.clone(),
            self.keypair.as_ref(),
        )
    }
}
