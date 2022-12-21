use clap::__macro_refs::once_cell;
use coset::CborSerializable;
use many_client::client::blocking::block_on;
use many_error::ManyError;
use many_identity::{Address, AnonymousIdentity};
use many_modules::r#async::{StatusArgs, StatusReturn};
use many_modules::{abci_frontend, blockchain, r#async};
use many_protocol::{encode_cose_sign1_from_response, ResponseMessage};
use many_types::blockchain::{
    Block, BlockIdentifier, SingleBlockQuery, SingleTransactionQuery, Transaction,
    TransactionIdentifier,
};
use many_types::{blockchain::RangeBlockQuery, SortOrder, Timestamp};
use once_cell::sync::Lazy;
use sha2::Digest;
use std::{
    borrow::Borrow,
    ops::{Bound, RangeBounds},
};
use tendermint::Time;
use tendermint_rpc::{query::Query, Client, Order};

const MAXIMUM_BLOCK_COUNT: u64 = 100;
static DEFAULT_BLOCK_LIST_QUERY: Lazy<Query> = Lazy::new(|| Query::gte("block.height", 0));

fn _many_block_from_tendermint_block<C: Client + Sync>(
    block: tendermint::Block,
    args: impl Borrow<blockchain::ListArgs>,
    client: &C,
) -> Result<Block, ManyError> {
    let (count, order, query) = transform_list_args(args.borrow())?;
    let transaction_results_by_id = tx_results(client, count, order, query)?;
    let height = block.header.height.value();
    let txs_count = block.data.len() as u64;
    let txs = block
        .data
        .into_iter()
        .map(|b| {
            let id = TransactionIdentifier { hash: hash_tx(&b) };
            Transaction {
                id: id.clone(),
                request: Some(b),
                response: transaction_results_by_id
                    .iter()
                    .find(|(txn_id, _)| *txn_id == id)
                    .map(|(_, result)| result)
                    .cloned(),
            }
        })
        .collect();
    Ok(Block {
        id: BlockIdentifier {
            hash: block.header.hash().into(),
            height,
        },
        parent: if height <= 1 {
            BlockIdentifier::genesis()
        } else {
            BlockIdentifier::new(block.header.last_block_id.unwrap().hash.into(), height - 1)
        },
        app_hash: Some(block.header.app_hash.value()),
        timestamp: Timestamp::new(
            block
                .header
                .time
                .duration_since(Time::unix_epoch())
                .unwrap()
                .as_secs(),
        )
        .unwrap(),
        txs_count,
        txs,
    })
}

fn hash_tx(tx: impl AsRef<[u8]>) -> Vec<u8> {
    let mut hasher = sha2::Sha256::new();
    hasher.update(tx.as_ref());
    hasher.finalize().to_vec()
}

fn _tm_order_from_many_order(order: impl Borrow<SortOrder>) -> tendermint_rpc::Order {
    match order.borrow() {
        SortOrder::Ascending => tendermint_rpc::Order::Ascending,
        SortOrder::Descending => tendermint_rpc::Order::Descending,
        _ => tendermint_rpc::Order::Ascending,
    }
}

fn _tm_query_from_many_filter(
    filter: impl Borrow<RangeBlockQuery>,
) -> Result<tendermint_rpc::query::Query, ManyError> {
    let mut query = tendermint_rpc::query::Query::default();
    query = match filter.borrow() {
        RangeBlockQuery::Height(range) => {
            query = match range.start_bound() {
                Bound::Included(x) => query.and_gte("block.height", *x),
                Bound::Excluded(x) => query.and_gt("block.height", *x),
                _ => query,
            };
            query = match range.end_bound() {
                Bound::Included(x) => query.and_lte("block.height", *x),
                Bound::Excluded(x) => query.and_lt("block.height", *x),
                _ => query,
            };
            query
        }
        RangeBlockQuery::Time(_range) => return Err(ManyError::unknown("Unimplemented")),
    };

    // The default query returns an error (TM 0.35)
    // Return all blocks
    // TODO: Test on TM 0.34 and report an issue in TM-rs if reproducible
    if query.to_string().is_empty() {
        query = DEFAULT_BLOCK_LIST_QUERY.clone();
    }

    Ok(query)
}

fn transform_list_args(
    blockchain::ListArgs {
        count,
        order,
        filter,
    }: &blockchain::ListArgs,
) -> Result<(u64, Order, Query), ManyError> {
    filter
        .as_ref()
        .map_or(
            Ok(DEFAULT_BLOCK_LIST_QUERY.clone()),
            _tm_query_from_many_filter,
        )
        .map(|filter| {
            (
                count.map_or(MAXIMUM_BLOCK_COUNT, |c| {
                    std::cmp::min(c, MAXIMUM_BLOCK_COUNT)
                }),
                order
                    .as_ref()
                    .map_or(tendermint_rpc::Order::Ascending, _tm_order_from_many_order),
                filter,
            )
        })
}

fn create_pagination(count: u64) -> Result<(u32, u8), ManyError> {
    // We can get maximum u8::MAX blocks per page and a maximum of u32::MAX pages
    // Find the correct number of pages and count
    let maximum_8_bit_integer: u64 = u8::MAX.into();
    Ok((
        num_integer::div_ceil(count, maximum_8_bit_integer)
            .try_into()
            .map_err(|_| ManyError::unknown("Unable to cast u64 to u32"))?,
        std::cmp::max(count, maximum_8_bit_integer)
            .try_into()
            .map_err(|_| ManyError::unknown("Unable to cast u64 to u8"))?,
    ))
}

fn tx_results<C: Client + Sync>(
    client: &C,
    count: u64,
    order: Order,
    query: Query,
) -> Result<Vec<(TransactionIdentifier, Vec<u8>)>, ManyError> {
    use futures_util::future::TryFutureExt;
    let (pages, count) = create_pagination(count)?;
    block_on(
        client
            .tx_search(query, true, pages, count, order)
            .map_err(|_| ManyError::unknown("Transaction search query returned an error"))
            .and_then(|response| async move {
                Ok(response
                    .txs
                    .iter()
                    .map(|tx| {
                        (
                            TransactionIdentifier {
                                hash: hash_tx(&tx.tx),
                            },
                            tx.tx_result.data.value().clone(),
                        )
                    })
                    .collect())
            }),
    )
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
            block_on(async {
                match self
                    .client
                    .tx(tendermint_rpc::abci::transaction::Hash::new(hash), false)
                    .await
                {
                    Ok(tx) => {
                        tracing::warn!("result: {}", hex::encode(tx.tx_result.data.value()));
                        Ok(StatusReturn::Done {
                            response: Box::new(
                                encode_cose_sign1_from_response(
                                    ResponseMessage::from_bytes(tx.tx_result.data.value())
                                        .map_err(abci_frontend::abci_transport_error)?,
                                    &AnonymousIdentity,
                                )
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
        let status = block_on(async { self.client.status().await }).map_err(|e| {
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

    fn transaction(
        &self,
        args: blockchain::TransactionArgs,
    ) -> Result<blockchain::TransactionReturns, ManyError> {
        let block = block_on(async {
            match args.query {
                SingleTransactionQuery::Hash(hash) => {
                    if let Ok(hash) = TryInto::<[u8; 32]>::try_into(hash) {
                        self.client
                            .tx(tendermint_rpc::abci::transaction::Hash::new(hash), true)
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
                request: None,
                response: None,
            },
        })
    }

    fn block(&self, args: blockchain::BlockArgs) -> Result<blockchain::BlockReturns, ManyError> {
        let block = block_on(async {
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
            let block = _many_block_from_tendermint_block(
                block,
                blockchain::ListArgs {
                    count: None,
                    order: None,
                    filter: None,
                },
                &self.client,
            )?;
            Ok(blockchain::BlockReturns { block })
        } else {
            Err(blockchain::unknown_block())
        }
    }

    fn list(&self, args: blockchain::ListArgs) -> Result<blockchain::ListReturns, ManyError> {
        let args_for_transactions = args.clone();
        let blockchain::ListArgs {
            count,
            order,
            filter,
        } = args;

        let count = count.map_or(MAXIMUM_BLOCK_COUNT, |c| {
            std::cmp::min(c, MAXIMUM_BLOCK_COUNT)
        });

        let (pages, count) = create_pagination(count)?;

        let order = order.map_or(tendermint_rpc::Order::Ascending, _tm_order_from_many_order);

        let query = filter.map_or(
            Ok(DEFAULT_BLOCK_LIST_QUERY.clone()),
            _tm_query_from_many_filter,
        )?;

        let (status, block) = block_on(async move {
            let status = self.client.status().await;
            let block = self.client.block_search(query, pages, count, order).await;
            (status, block)
        });

        let blocks = block
            .map_err(ManyError::unknown)?
            .blocks
            .into_iter()
            .map(|x| {
                _many_block_from_tendermint_block(x.block, &args_for_transactions, &self.client)
            })
            .collect::<Result<_, _>>()?;

        Ok(blockchain::ListReturns {
            height: status
                .map_err(ManyError::unknown)?
                .sync_info
                .latest_block_height
                .value(),
            blocks,
        })
    }

    fn request(
        &self,
        args: blockchain::RequestArgs,
    ) -> Result<blockchain::RequestReturns, ManyError> {
        let tx = block_on(async {
            match args.query {
                SingleTransactionQuery::Hash(hash) => {
                    if let Ok(hash) = TryInto::<[u8; 32]>::try_into(hash) {
                        self.client
                            .tx(tendermint_rpc::abci::transaction::Hash::new(hash), true)
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

        tracing::debug!("blockchain.request: {}", hex::encode(tx.tx.as_bytes()));

        Ok(blockchain::RequestReturns {
            request: tx.tx.as_bytes().to_vec(),
        })
    }

    fn response(
        &self,
        args: blockchain::ResponseArgs,
    ) -> Result<blockchain::ResponseReturns, ManyError> {
        let tx = block_on(async {
            match args.query {
                SingleTransactionQuery::Hash(hash) => {
                    if let Ok(hash) = TryInto::<[u8; 32]>::try_into(hash) {
                        self.client
                            .tx(tendermint_rpc::abci::transaction::Hash::new(hash), true)
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

        tracing::debug!(
            "blockchain.response: {}",
            hex::encode(tx.tx_result.data.value())
        );
        let response: ResponseMessage = minicbor::decode(tx.tx_result.data.value())
            .map_err(ManyError::deserialization_error)?;
        Ok(blockchain::ResponseReturns {
            response: encode_cose_sign1_from_response(response, &AnonymousIdentity)?
                .to_vec()
                .map_err(ManyError::serialization_error)?,
        })
    }
}
