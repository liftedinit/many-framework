use crate::types::{AbciBlock, AbciCommitInfo, AbciInfo, AbciInit};
use async_trait::async_trait;
use minicbor::decode;
use omni::message::{RequestMessage, ResponseMessage};
use omni::protocol::Attribute;
use omni::server::module::OmniModuleInfo;
use omni::{OmniError, OmniModule};
use std::sync::{Arc, Mutex};

pub const ABCI_SERVER: Attribute = Attribute::id(1000);

pub trait OmniAbciModuleBackend: std::fmt::Debug + Send + Sync {
    /// Called when the ABCI frontend is initialized. No action should be taken here, only
    /// information should be returned. If the ABCI frontend is restarted, this method
    /// will be called again.
    fn init(&mut self) -> AbciInit;

    // -- LIFECYCLE METHODS --
    /// Called at Genesis of the Tendermint blockchain.
    fn init_chain(&mut self) -> Result<(), OmniError>;

    /// Called at the start of a block.
    fn block_begin(&mut self, _info: AbciBlock) -> Result<(), OmniError> {
        Ok(())
    }

    /// Called when info is needed from the backend.
    fn info(&self) -> Result<AbciInfo, OmniError>;

    /// Called at the end of a block.
    fn block_end(&mut self) -> Result<(), OmniError> {
        Ok(())
    }

    /// Called after a block. The app should take this call and serialize its state.
    fn commit(&mut self) -> Result<AbciCommitInfo, OmniError>;
}

/// A module that adapt an OMNI application to an ABCI-OMNI bridge.
/// This module takes a backend (another module) which ALSO implements the ModuleBackend
/// trait, and exposes the `abci.info` and `abci.init` endpoints.
/// This module should only be exposed to the tendermint server's network. It is not
/// considered secure (just like an ABCI app would not).
#[derive(Debug, Clone)]
pub struct AbciModule<B: OmniAbciModuleBackend> {
    backend: Arc<Mutex<B>>,
    module_info: OmniModuleInfo,
}

impl<B: OmniAbciModuleBackend> AbciModule<B> {
    // TODO: remove dead_code by splitting omni-abci library and CLI into separate packages.
    #[allow(dead_code)]
    pub fn new(backend: Arc<Mutex<B>>, name: String) -> Self {
        let module_info = OmniModuleInfo {
            name,
            attributes: vec![ABCI_SERVER],
            endpoints: vec![
                "abci.info".to_string(),
                "abci.init".to_string(),
                "abci.initChain".to_string(),
                "abci.commit".to_string(),
                "abci.beginBlock".to_string(),
                "abci.endBlock".to_string(),
            ],
        };

        Self {
            backend,
            module_info,
        }
    }

    fn abci_init(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let mut backend = self.backend.lock().unwrap();
        Ok(ResponseMessage::from_request(
            &message,
            &message.to,
            minicbor::to_vec(backend.init())
                .map_err(|e| OmniError::serialization_error(e.to_string())),
        ))
    }
    fn abci_info(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let backend = self.backend.lock().unwrap();
        let info = backend.info()?;
        let bytes =
            minicbor::to_vec(info).map_err(|e| OmniError::serialization_error(e.to_string()))?;

        Ok(ResponseMessage::from_request(
            &message,
            &message.to,
            Ok(bytes),
        ))
    }

    fn abci_commit(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let mut backend = self.backend.lock().unwrap();
        let info = backend.commit()?;
        Ok(ResponseMessage::from_request(
            &message,
            &message.to,
            Ok(minicbor::to_vec(info)
                .map_err(|e| OmniError::deserialization_error(e.to_string()))?),
        ))
    }

    fn abci_begin_block(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let mut backend = self.backend.lock().unwrap();
        let info: AbciBlock =
            decode(&message.data).map_err(|e| OmniError::deserialization_error(e.to_string()))?;
        let result = backend.block_begin(info)?;

        Ok(ResponseMessage::from_request(
            &message,
            &message.to,
            Ok(minicbor::to_vec(result)
                .map_err(|e| OmniError::deserialization_error(e.to_string()))?),
        ))
    }
}

#[async_trait]
impl<B: OmniAbciModuleBackend> OmniModule for AbciModule<B> {
    #[inline]
    fn info(&self) -> &OmniModuleInfo {
        &self.module_info
    }

    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        match message.method.as_str() {
            "abci.info" => Ok(()),
            "abci.init" => Ok(()),
            "abci.initChain" => Ok(()),
            "abci.commit" => Ok(()),
            "abci.beginBlock" => Ok(()),
            "abci.endBlock" => Ok(()),
            x => Err(OmniError::invalid_method_name(x.to_string())),
        }
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        match message.method.as_str() {
            "abci.init" => self.abci_init(message),
            "abci.info" => self.abci_info(message),
            "abci.commit" => self.abci_commit(message),
            "abci.beginBlock" => self.abci_begin_block(message),
            "abci.endBlock" => Err(OmniError::internal_server_error()),
            x => Err(OmniError::invalid_method_name(x.to_string())),
        }
    }
}
