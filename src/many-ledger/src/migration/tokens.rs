use crate::error;
use crate::migration::MIGRATIONS;
use crate::storage::ledger_tokens::{key_for_ext_info, key_for_symbol};
use crate::storage::{SYMBOLS, SYMBOLS_BYTES};
use linkme::distributed_slice;
use many_error::ManyError;
use many_identity::Address;
use many_migration::InnerMigration;
use many_modules::ledger::extended_info::TokenExtendedInfo;
use many_types::ledger::{Symbol, TokenAmount, TokenInfo, TokenInfoSummary, TokenInfoSupply};
use merk::Op;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;

/// Migrate the subresource counter from "/config/account_id" to "/config/subresource_id"
fn migrate_subresource_counter(storage: &mut merk::Merk) -> Result<(), ManyError> {
    // Is the old counter present in the DB?
    let old_counter = storage
        .get(b"/config/account_id")
        .map_err(|_| ManyError::unknown("Unable to retrieve old counter"))?;

    // Is the new counter present in the DB?
    let new_counter = storage
        .get(b"/config/subresource_id")
        .map_err(|_| ManyError::unknown("Unable to retrieve new counter"))?;

    match (old_counter, new_counter) {
        // Old counter is present, new counter is not. First time running the migration.
        (Some(x), None) => {
            // Migrate the old counter to the new location in the database
            storage
                .apply(&[(b"/config/subresource_id".to_vec(), Op::Put(x))])
                .map_err(error::storage_apply_failed)?;

            // Delete the old counter from the DB
            storage
                .apply(&[(b"/config/account_id".to_vec(), Op::Delete)])
                .map_err(error::storage_apply_failed)?;
        }
        // No counter found. Set the new counter to 0.
        (None, None) => {
            storage
                .apply(&[(b"/config/subresource_id".to_vec(), Op::Put(vec![0u8; 4]))])
                .map_err(error::storage_apply_failed)?;
        }
        // Old counter is not present, new counter is present.
        // The migration did run in the past.
        // Skip this step
        (None, Some(_)) => {}
        // Both counters are present in the DB.
        // Something wrong is happening
        (Some(_), Some(_)) => {
            return Err(ManyError::unknown(
                "Two subresource counters found in the store; aborting",
            ))
        }
    }
    Ok(())
}

fn migrate_token_metadata(
    storage: &mut merk::Merk,
    extra: &HashMap<String, Value>,
) -> Result<(), ManyError> {
    // Make sure we have all the parameters we need for this migration
    let params = [
        "symbol",
        "symbol_name",
        "symbol_decimals",
        "symbol_total",       // TODO: This field could be calculated from the storage
        "symbol_circulating", // TODO: This field could be calculated from the storage
        "symbol_maximum",
        "symbol_owner",
    ];
    for param in params {
        if !extra.contains_key(param) {
            return Err(ManyError::unknown(format!(
                "Missing extra parameter '{param}' for Token Migration"
            )));
        }
    }

    let symbol: String = serde_json::from_value(extra["symbol"].clone())
        .map_err(ManyError::deserialization_error)?;
    let symbol = Symbol::from_str(&symbol)?;

    // Get symbol list from DB
    let symbol_and_ticker_enc = storage
        .get(SYMBOLS_BYTES)
        .map_err(error::storage_get_failed)?
        .ok_or_else(|| error::storage_key_not_found(SYMBOLS))?;

    let symbol_and_ticker: BTreeMap<Address, String> =
        minicbor::decode(&symbol_and_ticker_enc).map_err(ManyError::deserialization_error)?;

    // Get the symbol ticker from symbol list
    let ticker = symbol_and_ticker
        .get(&symbol)
        .ok_or_else(|| ManyError::unknown(format!("Symbol {symbol} not found in DB")))
        .cloned()?;

    let owner: Option<Address> = serde_json::from_value(extra["symbol_owner"].clone())
        .map_err(ManyError::deserialization_error)?;
    let maximum: Option<TokenAmount> = serde_json::from_value(extra["symbol_maximum"].clone())
        .map_err(ManyError::deserialization_error)?;
    let name: String = serde_json::from_value(extra["symbol_name"].clone())
        .map_err(ManyError::deserialization_error)?;
    let decimals: u64 = serde_json::from_value(extra["symbol_decimals"].clone())
        .map_err(ManyError::deserialization_error)?;
    let total: TokenAmount = serde_json::from_value(extra["symbol_total"].clone())
        .map_err(ManyError::deserialization_error)?;
    let circulating: TokenAmount = serde_json::from_value(extra["symbol_circulating"].clone())
        .map_err(ManyError::deserialization_error)?;
    let info = TokenInfo {
        symbol,
        summary: TokenInfoSummary {
            name,
            ticker,
            decimals,
        },
        supply: TokenInfoSupply {
            total,
            circulating,
            maximum,
        },
        owner,
    };

    // Add token metadata to the DB
    storage
        .apply(&[(
            key_for_symbol(&symbol),
            Op::Put(minicbor::to_vec(info).map_err(ManyError::serialization_error)?),
        )])
        .map_err(ManyError::unknown)?;
    storage
        .apply(&[(
            key_for_ext_info(&symbol),
            Op::Put(
                minicbor::to_vec(TokenExtendedInfo::default())
                    .map_err(ManyError::serialization_error)?,
            ),
        )])
        .map_err(ManyError::unknown)?;

    Ok(())
}

fn initialize(storage: &mut merk::Merk, extra: &HashMap<String, Value>) -> Result<(), ManyError> {
    migrate_subresource_counter(storage)?;
    migrate_token_metadata(storage, extra)?;

    Ok(())
}

#[distributed_slice(MIGRATIONS)]
pub static TOKEN_MIGRATION: InnerMigration<merk::Merk, ManyError> = InnerMigration::new_initialize(
    initialize,
    "Token Migration",
    "Move the database to new subresource counter and new token metadata",
);
