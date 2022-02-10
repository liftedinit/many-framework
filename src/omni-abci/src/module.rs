use omni::server::module::{abci_frontend, blockchain};
use omni::types::blockchain::{
    Block, BlockIdentifier, SingleBlockQuery, Transaction, TransactionIdentifier,
};
use omni::types::Timestamp;
use omni::OmniError;
use tendermint::Time;
use tendermint_rpc::query::Query;
use tendermint_rpc::{Client, Order};

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
        let status = smol::block_on(async { self.client.status().await }).map_err(|e| {
            tracing::error!("abci transport: {}", e.to_string());
            abci_frontend::abci_transport_error(e.to_string())
        })?;

        Ok(blockchain::InfoReturns {
            latest_block: BlockIdentifier {
                hash: status.sync_info.latest_block_hash.as_bytes().to_vec(),
                height: status.sync_info.latest_block_height.value(),
            },
            app_hash: Some(status.sync_info.latest_app_hash.value().to_vec()),
            retained_height: None,
        })
    }

    fn block(&self, args: blockchain::BlockArgs) -> Result<blockchain::BlockReturns, OmniError> {
        let block = smol::block_on(async {
            match args.query {
                SingleBlockQuery::Hash(hash) => self
                    .client
                    .block_search(
                        Query::eq("hash", format!("0x{}", hex::encode(hash.as_slice()))),
                        0,
                        1,
                        Order::Ascending,
                    )
                    .await
                    .map_err(|e| {
                        tracing::error!("abci transport: {}", e.to_string());
                        abci_frontend::abci_transport_error(e.to_string())
                    })
                    .map(|search| search.blocks.into_iter().take(1).map(|b| b.block).next()),
                SingleBlockQuery::Height(height) => self
                    .client
                    .block(height as u32)
                    .await
                    .map_err(|e| {
                        tracing::error!("abci transport: {}", e.to_string());
                        abci_frontend::abci_transport_error(e.to_string())
                    })
                    .map(|x| Some(x.block)),
            }
        })?;

        if let Some(block) = block {
            let height = block.header.height.value();
            let txs_count = block.data.len() as u64;
            let txs = block
                .data
                .into_iter()
                .map(|b| {
                    use sha2::Digest;
                    let mut hasher = sha2::Sha256::new();
                    hasher.update(&b);
                    Transaction {
                        id: TransactionIdentifier {
                            hash: hasher.finalize().to_vec(),
                        },
                        content: Some(b),
                    }
                })
                .collect();
            Ok(blockchain::BlockReturns {
                block: Block {
                    id: BlockIdentifier {
                        hash: block.header.hash().into(),
                        height,
                    },
                    parent: if height <= 1 {
                        BlockIdentifier::genesis()
                    } else {
                        BlockIdentifier::new(
                            block.header.last_block_id.unwrap().hash.into(),
                            height - 1,
                        )
                    },
                    timestamp: Timestamp::new(
                        block
                            .header
                            .time
                            .duration_since(Time::unix_epoch())
                            .unwrap()
                            .as_secs() as u64,
                    )
                    .unwrap(),
                    txs_count,
                    txs,
                },
            })
        } else {
            Err(blockchain::unknown_block())
        }
    }
}
