use crate::message::{OmniError, RequestMessage, ResponseMessage};
use crate::Identity;
use async_trait::async_trait;
use minicose::{CoseKey, CoseSign1, Ed25519CoseKeyBuilder};
use ring::signature::{Ed25519KeyPair, KeyPair};
use std::fmt::Debug;

#[async_trait]
pub trait LowLevelOmniRequestHandler: Send + Sync + Debug {
    async fn execute(&self, envelope: CoseSign1) -> Result<CoseSign1, String>;
}

#[derive(Debug)]
pub struct HandlerExecutorAdapter<H: OmniRequestHandler + Debug> {
    handler: H,
    identity: Identity,
    keypair: Option<Ed25519KeyPair>,
}

impl<H: OmniRequestHandler + Debug> HandlerExecutorAdapter<H> {
    pub fn new(handler: H, identity: Identity, keypair: Option<Ed25519KeyPair>) -> Self {
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
            handler,
            identity,
            keypair,
        }
    }
}

#[async_trait]
impl<H: OmniRequestHandler + Debug> LowLevelOmniRequestHandler for HandlerExecutorAdapter<H> {
    async fn execute(&self, envelope: CoseSign1) -> Result<CoseSign1, String> {
        let request = crate::message::decode_request_from_cose_sign1(envelope)
            .and_then(|message| self.handler.validate(&message).map(|_| message));

        let response = match request {
            Ok(x) => match self.handler.execute(x).await {
                Err(e) => ResponseMessage::error(&self.identity, e),
                Ok(x) => x,
            },
            Err(e) => ResponseMessage::error(&self.identity, e),
        };

        crate::message::encode_cose_sign1_from_response(
            response,
            self.identity.clone(),
            self.keypair.as_ref(),
        )
    }
}

/// A simpler version of the [OmniRequestHandler] which only deals with methods and payloads.
#[async_trait]
pub trait SimpleRequestHandler: Send + Sync + Debug {
    fn validate(&self, _method: &str, _payload: &[u8]) -> Result<(), OmniError> {
        Ok(())
    }

    async fn handle(&self, method: &str, payload: &[u8]) -> Result<Vec<u8>, OmniError>;
}

#[async_trait]
pub trait OmniRequestHandler: Send + Sync + Debug {
    /// Validate that a message is okay with us.
    fn validate(&self, _message: &RequestMessage) -> Result<(), OmniError> {
        Ok(())
    }

    /// Handle an incoming request message, and returns the response message.
    /// This cannot fail. It should instead responds with a proper error response message.
    /// See the spec.
    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError>;
}

#[derive(Debug)]
pub struct SimpleRequestHandlerAdapter<I: SimpleRequestHandler>(pub I);

#[async_trait]
impl<I: SimpleRequestHandler> OmniRequestHandler for SimpleRequestHandlerAdapter<I> {
    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        self.0
            .validate(message.method.as_str(), message.data.as_slice())
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let payload = self
            .0
            .handle(message.method.as_str(), message.data.as_slice())
            .await;

        Ok(ResponseMessage {
            version: Some(1),
            from: message.to,
            data: payload,
            to: message.from,
            timestamp: None,
            id: message.id,
        })
    }
}

pub mod http;
