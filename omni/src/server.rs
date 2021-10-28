use crate::message::{RequestMessage, ResponseMessage};
use crate::protocol::Status;
use crate::transport::OmniRequestHandler;
use crate::{Identity, OmniError};
use async_trait::async_trait;
use ring::signature::Ed25519KeyPair;

pub mod module;
pub mod namespace;

pub use module::ModuleRequestHandler;
pub use namespace::NamespacedRequestHandler;

#[derive(Debug)]
struct BaseServerModule {
    pub status: Status,
    pub handler: ModuleRequestHandler,
}

impl Default for BaseServerModule {
    fn default() -> Self {}
}

#[async_trait]
impl OmniRequestHandler for BaseServerModule {
    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        self.handler.validate(message)
    }

    async fn execute(&self, message: &RequestMessage) -> Result<ResponseMessage, OmniError> {
        self.handler.execute(message).await
    }
}

#[derive(Debug)]
pub struct OmniServer {
    namespace: NamespacedRequestHandler,
    identity: Identity,
    keypair: Ed25519KeyPair,
}

impl OmniServer {
    pub fn new(identity: Identity, keypair: ring::signature::Ed25519KeyPair) -> Self {
        debug_assert!(identity.is_addressable());
        Self {
            identity,
            keypair,
            namespace: NamespacedRequestHandler::new(BaseServerModule::default()),
        }
    }

    pub fn with_namespace<NS, H>(&mut self, namespace: NS, handler: H) -> &mut Self
    where
        NS: ToString,
        H: OmniRequestHandler + 'static,
    {
        self.namespace.with_namespace(namespace, handler);
        self
    }
}

#[async_trait]
impl OmniRequestHandler for OmniServer {
    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        let to = message.to;
        // Verify that the message is for this server.
        if &self.identity != &to {
            Err(OmniError::unknown_destination(
                to.to_string(),
                self.identity.to_string(),
            ))
        } else {
            self.namespace.validate(message)
        }
    }
    async fn execute(&self, message: &RequestMessage) -> Result<ResponseMessage, OmniError> {
        self.namespace.execute(message).await.map(|mut r| {
            r.from = self.identity;
            r
        })
    }
}
