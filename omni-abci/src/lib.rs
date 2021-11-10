use async_trait::async_trait;
use minicbor::Encoder;
use omni::message::{RequestMessage, ResponseMessage};
use omni::protocol::Attribute;
use omni::server::module::{OmniModule, OmniModuleInfo};
use omni::OmniError;
use std::fmt::Debug;
use std::sync::Arc;

pub mod abci_app;
pub mod omni_app;

pub const ABCI_SERVER: Attribute = Attribute::new(1000, &["abci.info"]);

pub trait OmniAbciModuleBackend {
    fn query_methods(&self) -> Result<Vec<String>, OmniError>;
    fn height(&self) -> Result<u64, OmniError>;
    fn hash(&self) -> Result<Vec<u8>, OmniError>;
    fn commit(&self) -> Result<(), OmniError>;
}

/// A module that adapt an OMNI application to an ABCI-OMNI bridge.
/// This module takes a backend (another module) which ALSO implements the ModuleBackend
/// trait, and exposes the `abci.info` and `abci.init` endpoints.
#[derive(Debug, Clone)]
pub struct AbciModule<B: OmniModule + OmniAbciModuleBackend> {
    backend: Arc<B>,
    module_info: OmniModuleInfo,
}

impl<B: OmniModule + OmniAbciModuleBackend> AbciModule<B> {
    pub fn new(backend: B) -> Self {
        let module_info = OmniModuleInfo {
            name: format!("abci-{}", backend.info().name),
            attributes: [vec![ABCI_SERVER], backend.info().attributes.clone()]
                .concat()
                .to_vec(),
        };

        Self {
            backend: Arc::new(backend),
            module_info,
        }
    }

    fn abci_info(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let mut bytes = Vec::with_capacity(128);
        let mut e = Encoder::new(&mut bytes);

        let (queries, height, hash) = {
            let backend = &self.backend;
            (backend.query_methods()?, backend.height()?, backend.hash()?)
        };

        e.begin_map()
            .and_then(move |e| {
                e.str("queries")?;
                e.array(queries.len() as u64)?;
                for i in queries {
                    e.str(&i)?;
                }

                e.str("height")?.u64(height)?;
                e.str("hash")?.bytes(hash.as_slice())?;

                e.end()
            })
            .map_err(|e| OmniError::serialization_error(e.to_string()))?;

        Ok(ResponseMessage::from_request(
            &message,
            &message.to,
            Ok(bytes),
        ))
    }

    fn commit(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let mut backend = &self.backend;
        backend.commit();
        Ok(ResponseMessage::from_request(
            &message,
            &message.to,
            Ok(Vec::new()),
        ))
    }
}

#[async_trait]
impl<B: OmniModule + OmniAbciModuleBackend> OmniModule for AbciModule<B> {
    #[inline]
    fn info(&self) -> &OmniModuleInfo {
        &self.module_info
    }

    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        match message.method.as_str() {
            "abci.info" => Ok(()),
            "abci.commit" => Ok(()),
            _ => self.backend.validate(message),
        }
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        match message.method.as_str() {
            "abci.info" => self.abci_info(message),
            "abci.commit" => self.commit(message),
            _ => { self.backend.execute(message) }.await,
        }
    }
}
