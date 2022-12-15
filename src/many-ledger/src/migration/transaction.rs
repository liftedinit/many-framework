use crate::migration::MIGRATIONS;
use crate::storage::event::LedgerIterator;
use linkme::distributed_slice;
use many_error::ManyError;
use many_migration::InnerMigration;
use many_modules::account::features::multisig::InfoReturn;
use many_modules::events::{EventInfo, EventLog};
use many_types::{blockchain::{Block, Transaction}, Memo, SortOrder};
use merk::Op;

fn initialize(storage: &mut merk::Merk) -> Result<(), ManyError> {
    LedgerIterator::all_blocks(storage)
        .map(|block| {
            block.map_err(ManyError::unknown).map(|(key, value)| {
                minicbor::decode::<Block>(value.as_slice())
                    .map_err(ManyError::deserialization_error)
                    .map(|Block {
                        id,
                        parent,
                        app_hash,
                        timestamp,
                        txs_count,
                        txs,
                    }| (key, Block {
                        id,
                        parent,
                        app_hash,
                        timestamp,
                        txs_count,
                        txs: txs.into_iter().map(|Transaction {
                            id,
                            content_,
                            request,
                            response
                        }| Transaction {
                            id,
                            content_: None,
                            request: content_.and_then(|bytes| minicbor::decode(bytes.as_ref()).ok()),
                            response
                        }).collect(),
                    }))
            })
        })
        .for_each(|_| ());
    Ok(())
}

#[distributed_slice(MIGRATIONS)]
pub static TRANSACTION_MIGRATION: InnerMigration<merk::Merk, ManyError> =
    InnerMigration::new_initialize(
        initialize,
        "Transaction Migration",
        "Move the database from legacy transaction type to the new transaction data type.",
    );
