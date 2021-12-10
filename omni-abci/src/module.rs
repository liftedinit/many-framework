use crate::info::AbciInfo;
use crate::init::AbciInit;
use async_trait::async_trait;
use minicbor::Encoder;
use omni::message::{RequestMessage, ResponseMessage};
use omni::protocol::Attribute;
use omni::server::module::OmniModuleInfo;
use omni::{OmniError, OmniModule};
use std::sync::Arc;

pub const ABCI_SERVER: Attribute = Attribute::new(
    1000,
    &[
        "abci.info",
        "abci.init",
        "abci.commit",
        "abci.beginBlock",
        "abci.endBlock",
    ],
);

pub trait OmniAbciModuleBackend: OmniModule {
    /// Called when the ABCI frontend is initialized. No action should be taken here, only
    /// information should be returned. If the ABCI frontend is restarted, this method
    /// will be called again.
    fn init(&self) -> AbciInit;

    // -- LIFECYCLE METHODS --
    /// Called at Genesis of the Tendermint blockchain.
    fn init_chain(&self) -> Result<(), OmniError>;

    /// Called at the start of a block.
    fn block_begin(&self) -> Result<(), OmniError> {
        Ok(())
    }

    /// Called when info is needed from the backend.
    fn info(&self) -> Result<AbciInfo, OmniError>;

    /// Called at the end of a block.
    fn block_end(&self) -> Result<(), OmniError> {
        Ok(())
    }

    /// Called after a block. The app should take this call and serialize its state.
    fn commit(&self) -> Result<(), OmniError>;
}

/// A module that adapt an OMNI application to an ABCI-OMNI bridge.
/// This module takes a backend (another module) which ALSO implements the ModuleBackend
/// trait, and exposes the `abci.info` and `abci.init` endpoints.
/// This module should only be exposed to the tendermint server's network. It is not
/// considered secure (just like an ABCI app would not).
#[derive(Debug, Clone)]
pub struct AbciModule<B: OmniAbciModuleBackend> {
    backend: Arc<B>,
    module_info: OmniModuleInfo,
}

impl<B: OmniAbciModuleBackend> AbciModule<B> {
    pub fn new(backend: B) -> Self {
        let backend_info = OmniModule::info(&backend);
        let module_info = OmniModuleInfo {
            name: format!("abci-{}", backend_info.name),
            attributes: [vec![ABCI_SERVER], backend_info.attributes.clone()]
                .concat()
                .to_vec(),
        };

        Self {
            backend: Arc::new(backend),
            module_info,
        }
    }

    fn abci_init(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        Ok(ResponseMessage::from_request(
            &message,
            &message.to,
            minicbor::to_vec(self.backend.init())
                .map_err(|e| OmniError::serialization_error(e.to_string())),
        ))
    }
    fn abci_info(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let mut bytes = Vec::with_capacity(128);
        let mut e = Encoder::new(&mut bytes);

        let info = OmniAbciModuleBackend::info(self.backend.as_ref())?;
        e.encode(info)
            .map_err(|e| OmniError::serialization_error(e.to_string()))?;

        Ok(ResponseMessage::from_request(
            &message,
            &message.to,
            Ok(bytes),
        ))
    }

    fn abci_commit(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        self.backend.commit()?;
        Ok(ResponseMessage::from_request(
            &message,
            &message.to,
            Ok(Vec::new()),
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
            "abci.init" => Ok(()),
            "abci.initChain" => Ok(()),
            "abci.info" => Ok(()),
            "abci.commit" => Ok(()),
            "abci.beginBlock" => Ok(()),
            "abci.endBlock" => Ok(()),
            _ => self.backend.validate(message),
        }
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        match message.method.as_str() {
            "abci.init" => self.abci_init(message),
            "abci.info" => self.abci_info(message),
            "abci.commit" => self.abci_commit(message),
            "abci.beginBlock" => Err(OmniError::internal_server_error()),
            "abci.endBlock" => Err(OmniError::internal_server_error()),
            _ => {
                // Forward the message to the backend. If we got here, the contract is the message
                // is a command message and have been through the blockchain.
                self.backend.execute(message).await
            }
        }
    }
}
