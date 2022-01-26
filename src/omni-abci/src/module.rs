use crate::types::{AbciBlock, AbciCommitInfo, AbciInfo, AbciInit};
use omni::OmniError;
use omni_module::omni_module;

/// A module that adapt an OMNI application to an ABCI-OMNI bridge.
/// This module takes a backend (another module) which ALSO implements the ModuleBackend
/// trait, and exposes the `abci.info` and `abci.init` endpoints.
/// This module should only be exposed to the tendermint server's network. It is not
/// considered secure (just like an ABCI app would not).
#[omni_module(name = AbciModule, id = 1000, namespace = abci)]
pub trait OmniAbciModuleBackend: std::fmt::Debug + Send + Sync {
    /// Called when the ABCI frontend is initialized. No action should be taken here, only
    /// information should be returned. If the ABCI frontend is restarted, this method
    /// will be called again.
    fn init(&mut self) -> Result<AbciInit, OmniError>;

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
