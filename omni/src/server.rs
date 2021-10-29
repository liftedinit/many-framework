use crate::message::{RequestMessage, ResponseMessage};
use crate::protocol::StatusBuilder;
use crate::transport::OmniRequestHandler;
use crate::{Identity, OmniError};
use async_trait::async_trait;
use minicose::{CoseKey, Ed25519CoseKeyBuilder};
use module::base::BaseServerModule;
use ring::signature::{Ed25519KeyPair, KeyPair};

pub mod function;
pub mod module;
pub mod namespace;

pub use namespace::NamespacedRequestHandler;

#[derive(Debug)]
pub struct OmniServer {
    namespace: NamespacedRequestHandler,
    identity: Identity,
}

impl OmniServer {
    pub fn new(identity: Identity, public_key: &Ed25519KeyPair) -> Self {
        debug_assert!(identity.is_addressable());

        let x = public_key.public_key().as_ref().to_vec();
        let public_key: CoseKey = Ed25519CoseKeyBuilder::default()
            .x(x)
            .build()
            .unwrap()
            .into();

        let status = StatusBuilder::default()
            .version(1)
            .public_key(public_key)
            .identity(identity)
            .internal_version(vec![])
            .attributes(vec![])
            .build()
            .unwrap();

        Self {
            identity,
            namespace: NamespacedRequestHandler::new(BaseServerModule::new(status)),
        }
    }

    pub fn with_namespace<NS, H>(mut self, namespace: NS, handler: H) -> Self
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
        eprintln!("to: {} is_anon: {}", to, to.is_anonymous());
        // Verify that the message is for this server, if it's not anonymous.
        if to.is_anonymous() || &self.identity == &to {
            self.namespace.validate(message)
        } else {
            Err(OmniError::unknown_destination(
                to.to_string(),
                self.identity.to_string(),
            ))
        }
    }
    async fn execute(&self, message: &RequestMessage) -> Result<ResponseMessage, OmniError> {
        self.namespace.execute(message).await.map(|mut r| {
            r.from = self.identity;
            r
        })
    }
}
