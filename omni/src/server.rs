use crate::message::{RequestMessage, ResponseMessage};
use crate::protocol::Status;
use crate::transport::OmniRequestHandler;
use crate::{Identity, OmniError};
use async_trait::async_trait;

pub mod module;
pub mod namespace;

pub use module::ModuleRequestHandler;
pub use namespace::NamespacedRequestHandler;

struct BaseServerModule {
    pub status: Status,
    pub handler: ModuleRequestHandler,
}

#[derive(Debug)]
pub struct Server {
    handler: NamespacedRequestHandler,
    identity: Identity,
    keypair: ring::signature::Ed25519KeyPair,
}

impl Server {
    pub fn new(identity: Identity, keypair: ring::signature::Ed25519KeyPair) -> Self {
        debug_assert!(identity.is_addressable());
        Self {
            identity,
            keypair,
            handler: Default::default(),
        }
    }

    pub fn with_namespace<NS, H>(&mut self, namespace: NS, handler: H) -> &mut Self
    where
        NS: ToString,
        H: OmniRequestHandler + 'static,
    {
        self.handler.with_namespace(namespace, handler);
        self
    }
}

#[async_trait]
impl OmniRequestHandler for Server {
    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        let to = message.to;
        // Verify that the message is for this server.
        if &self.identity != &to {
            Err(OmniError::unknown_destination(
                to.to_string(),
                self.identity.to_string(),
            ))
        } else {
            self.handler.validate(message)
        }
    }
    async fn execute(&self, message: &RequestMessage) -> Result<ResponseMessage, OmniError> {
        self.handler.execute(message).await.map(|mut r| {
            r.from = self.identity;
            r
        })
    }
}
