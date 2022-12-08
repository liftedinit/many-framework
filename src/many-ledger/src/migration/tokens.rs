use crate::migration::MIGRATIONS;
use crate::storage::ledger_tokens::{key_for_ext_info, key_for_symbol};
use linkme::distributed_slice;
use many_error::ManyError;
use many_identity::Address;
use many_migration::InnerMigration;
use many_modules::ledger::extended_info::TokenExtendedInfo;
use many_types::ledger::{Symbol, TokenAmount, TokenInfo, TokenInfoSummary, TokenInfoSupply};
use merk::Op;
use std::str::FromStr;

fn initialize(storage: &mut merk::Merk) -> Result<(), ManyError> {
    // Move legacy subresource counter to new schema
    // Get the old counter
    let old_counter = storage
        .get(b"/config/account_id")
        .map_err(|_| ManyError::unknown("Unable to retrieve old counter"))?
        .ok_or_else(|| ManyError::unknown("Old counter doesn't exists"))?;

    // Migrate the old counter to the new location in the database
    storage
        .apply(&[(b"/config/subresource_id".to_vec(), Op::Put(old_counter))])
        .unwrap();

    let mfx = Symbol::from_str("mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz")
        .map_err(ManyError::unknown)?; // mfx

    let info = TokenInfo {
        symbol: mfx,
        summary: TokenInfoSummary {
            name: "Manifest Network Token".to_string(),
            ticker: "mfx".to_string(),
            decimals: 9,
        },
        supply: TokenInfoSupply {
            total: TokenAmount::from(100_000_000_000_000_000u64),
            circulating: TokenAmount::from(100_000_000_000_000_000u64),
            maximum: None,
        },
        owner: Some(Address::from_str(
            "mqbh742x4s356ddaryrxaowt4wxtlocekzpufodvowrirfrqaaaaa3l",
        )?),
    };

    // Add MFX token metadata
    storage
        .apply(&[(
            key_for_symbol(&mfx),
            Op::Put(minicbor::to_vec(info).map_err(ManyError::serialization_error)?),
        )])
        .map_err(ManyError::unknown)?;
    storage
        .apply(&[(
            key_for_ext_info(&mfx),
            Op::Put(
                minicbor::to_vec(TokenExtendedInfo::default())
                    .map_err(ManyError::serialization_error)?,
            ),
        )])
        .map_err(ManyError::unknown)?;

    Ok(())
}

#[distributed_slice(MIGRATIONS)]
pub static TOKEN_MIGRATION: InnerMigration<merk::Merk, ManyError> = InnerMigration::new_initialize(
    initialize,
    "Token Migration",
    "Move the database to new subresource counter and new token metadata",
);
