use minicbor::bytes::ByteVec;
use omni::server::module::{abci_frontend, blockchain};
use omni::OmniError;
use tendermint_rpc::Client;

pub struct AbciBlockchainModuleImpl<C: Client> {
    client: C,
}

impl<C: Client> AbciBlockchainModuleImpl<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }
}

impl<C: Client + Send + Sync> blockchain::BlockchainModuleBackend for AbciBlockchainModuleImpl<C> {
    fn info(&self) -> Result<blockchain::InfoReturns, OmniError> {
        tracing::info!("blockchain.info");
        let status = smol::block_on(async { self.client.status().await }).map_err(|e| {
            tracing::error!("abci transport: {}", e.to_string());
            abci_frontend::abci_transport_error(e.to_string())
        })?;

        Ok(blockchain::InfoReturns {
            hash: ByteVec::from(status.sync_info.latest_block_hash.as_bytes().to_vec()),
            app_hash: Some(ByteVec::from(
                status.sync_info.latest_app_hash.value().to_vec(),
            )),
            height: status.sync_info.latest_block_height.value(),
            retained_height: None,
        })
    }

    fn blocks(
        &self,
        _args: blockchain::BlocksArgs,
    ) -> Result<blockchain::BlocksReturns, OmniError> {
        todo!()
    }
}
