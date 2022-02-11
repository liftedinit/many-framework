use omni::server::module::{abci_frontend, blockchain};
use omni::types::blockchain::{
    Block, BlockIdentifier, SingleBlockQuery, Transaction, TransactionIdentifier,
};
use omni::types::Timestamp;
use omni::OmniError;
use tendermint::Time;
use tendermint_rpc::Client;

fn _omni_block_from_tendermint_block(block: tendermint::Block) -> Block {
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
    Block {
        id: BlockIdentifier {
            hash: block.header.hash().into(),
            height,
        },
        parent: if height <= 1 {
            BlockIdentifier::genesis()
        } else {
            BlockIdentifier::new(block.header.last_block_id.unwrap().hash.into(), height - 1)
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
    }
}

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
                SingleBlockQuery::Hash(hash) => {
                    if let Ok(hash) = TryInto::<[u8; 32]>::try_into(hash) {
                        self.client
                            .block_by_hash(tendermint::Hash::Sha256(hash))
                            .await
                            .map_err(|e| {
                                tracing::error!("abci transport: {}", e.to_string());
                                abci_frontend::abci_transport_error(e.to_string())
                            })
                            .map(|search| search.block)
                    } else {
                        Err(OmniError::unknown("Invalid hash length.".to_string()))
                    }
                }
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
            let block = _omni_block_from_tendermint_block(block);
            Ok(blockchain::BlockReturns { block })
        } else {
            Err(blockchain::unknown_block())
        }
    }
}
