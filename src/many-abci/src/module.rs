use many_error::ManyError;
use many_identity::Address;
use many_modules::r#async::{StatusArgs, StatusReturn};
use many_modules::{abci_frontend, blockchain, r#async};
use many_protocol::ResponseMessage;
use many_types::blockchain::{
    Block, BlockIdentifier, SingleBlockQuery, SingleTransactionQuery, Transaction,
    TransactionIdentifier,
};
use many_types::Timestamp;
use tendermint::Time;
use tendermint_rpc::Client;

fn _many_block_from_tendermint_block(block: tendermint::Block) -> Block {
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
        app_hash: Some(block.header.app_hash.as_bytes().to_vec()),
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

impl<C: Client> Drop for AbciBlockchainModuleImpl<C> {
    fn drop(&mut self) {
        tracing::info!("ABCI Blockchain Module being dropped.");
    }
}

impl<C: Client + Send + Sync> r#async::AsyncModuleBackend for AbciBlockchainModuleImpl<C> {
    fn status(&self, _sender: &Address, args: StatusArgs) -> Result<StatusReturn, ManyError> {
        let hash = args.token.as_ref();

        if let Ok(hash) = TryInto::<[u8; 32]>::try_into(hash) {
            smol::block_on(async {
                match self.client.tx(tendermint::Hash::Sha256(hash), false).await {
                    Ok(tx) => {
                        // Bse64 decode is required because of an issue in `tendermint-rs` 0.28.0
                        // TODO: Remove when https://github.com/informalsystems/tendermint-rs/issues/1251 is fixed
                        let cbor_tx_result_data = base64::decode(&tx.tx_result.data)
                            .map_err(ManyError::deserialization_error)?;
                        tracing::warn!("result: {}", hex::encode(&cbor_tx_result_data));
                        Ok(StatusReturn::Done {
                            response: Box::new(
                                ResponseMessage::from_bytes(&cbor_tx_result_data)
                                    .map_err(abci_frontend::abci_transport_error)?,
                            ),
                        })
                    }

                    Err(_) => Ok(StatusReturn::Unknown),
                }
            })
        } else {
            Err(ManyError::unknown("Invalid async token .".to_string()))
        }
    }
}

impl<C: Client + Send + Sync> blockchain::BlockchainModuleBackend for AbciBlockchainModuleImpl<C> {
    fn info(&self) -> Result<blockchain::InfoReturns, ManyError> {
        let status = smol::block_on(async { self.client.status().await }).map_err(|e| {
            tracing::error!("abci transport: {}", e.to_string());
            abci_frontend::abci_transport_error(e.to_string())
        })?;

        Ok(blockchain::InfoReturns {
            latest_block: BlockIdentifier {
                hash: status.sync_info.latest_block_hash.as_bytes().to_vec(),
                height: status.sync_info.latest_block_height.value(),
            },
            app_hash: Some(status.sync_info.latest_app_hash.as_bytes().to_vec()),
            retained_height: None,
        })
    }

    fn transaction(
        &self,
        args: blockchain::TransactionArgs,
    ) -> Result<blockchain::TransactionReturns, ManyError> {
        let block = smol::block_on(async {
            match args.query {
                SingleTransactionQuery::Hash(hash) => {
                    if let Ok(hash) = TryInto::<[u8; 32]>::try_into(hash) {
                        self.client
                            .tx(tendermint::Hash::Sha256(hash), true)
                            .await
                            .map_err(|e| {
                                tracing::error!("abci transport: {}", e.to_string());
                                abci_frontend::abci_transport_error(e.to_string())
                            })
                    } else {
                        Err(ManyError::unknown("Invalid transaction hash .".to_string()))
                    }
                }
            }
        })?;

        let tx_hash = block.hash.as_bytes().to_vec();
        Ok(blockchain::TransactionReturns {
            txn: Transaction {
                id: TransactionIdentifier { hash: tx_hash },
                content: None,
            },
        })
    }

    fn block(&self, args: blockchain::BlockArgs) -> Result<blockchain::BlockReturns, ManyError> {
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
                        Err(ManyError::unknown("Invalid hash length.".to_string()))
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
            let block = _many_block_from_tendermint_block(block);
            Ok(blockchain::BlockReturns { block })
        } else {
            Err(blockchain::unknown_block())
        }
    }
}
