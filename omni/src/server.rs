use crate::message::{RequestMessage, ResponseMessage};
use crate::protocol::StatusBuilder;
use crate::server::module::{OmniModule, OmniModuleInfo};
use crate::transport::OmniRequestHandler;
use crate::{Identity, OmniError};
use async_trait::async_trait;
use minicose::{CoseKey, Ed25519CoseKeyBuilder};
use ring::signature::{Ed25519KeyPair, KeyPair};
use std::collections::BTreeSet;

pub mod function;
pub mod module;

use crate::server::module::base::BaseServerModule;

#[derive(Debug, Default)]
pub struct OmniServer {
    modules: Vec<Box<dyn OmniModule>>,
    method_cache: BTreeSet<&'static str>,
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
            ..Default::default()
        }
        .with_module(BaseServerModule::new(status))
    }

    pub fn with_module<M>(mut self, module: M) -> Self
    where
        M: OmniModule + 'static,
    {
        let OmniModuleInfo { attributes, name } = module.info();
        for a in &attributes {
            let id = a.id;

            if let Some(m) = self
                .modules
                .iter()
                .find(|m| m.info().attributes.iter().any(|a| a.id == id))
            {
                panic!("Module {} already implements attribute {}.", name, id);
            }
        }

        for a in &attributes {
            for e in a.endpoints {
                if self.method_cache.contains(e) {
                    unreachable!(
                        "Method '{}' already implemented, but there was no attribute conflict.",
                        e
                    );
                }
            }
        }

        // Update the cache.
        for a in attributes {
            for e in a.endpoints {
                self.method_cache.insert(e);
            }
        }
        self.modules.push(Box::new(module));
        self
    }
}

#[async_trait]
impl OmniRequestHandler for OmniServer {
    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        let to = message.to;
        let method = message.method.as_str();

        // Verify that the message is for this server, if it's not anonymous.
        if to.is_anonymous() || &self.identity == &to {
            // Verify the endpoint.
            if self.method_cache.contains(method) {
                Ok(())
            } else {
                Err(OmniError::invalid_method_name(method.to_string()))
            }
        } else {
            Err(OmniError::unknown_destination(
                to.to_string(),
                self.identity.to_string(),
            ))
        }
    }
    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let method = &message.method.as_str();

        for m in &self.modules {
            let attrs = m.info().attributes;
            if attrs.iter().any(|a| a.endpoints.contains(method)) {
                return m.execute(message).await.map(|mut r| {
                    r.from = self.identity;
                    r
                });
            }
        }
        Err(OmniError::invalid_method_name(method.to_string()))
    }
}
