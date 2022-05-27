extern crate core;

use crate::storage::key_for_account_balance;
use many::types::ledger::{Symbol, TokenAmount};
use many::Identity;
use std::collections::BTreeMap;

pub mod error;
pub mod module;
pub mod storage;

/// Verify a proof.
pub fn verify_proof(
    bytes: &[u8],
    identity: &Identity,
    symbols: &[Symbol],
    expected_hash: &[u8; 32],
) -> Result<BTreeMap<Symbol, TokenAmount>, String> {
    let values = merk::verify(bytes, *expected_hash).map_err(|e| e.to_string())?;

    let mut result = BTreeMap::new();
    for symbol in symbols.iter() {
        let key = key_for_account_balance(identity, symbol);
        let amount = values.get(&key).map_err(|e| e.to_string())?;
        result.insert(
            *symbol,
            amount
                .as_ref()
                .map_or(TokenAmount::zero(), |x| TokenAmount::from(x.to_vec())),
        );
    }

    Ok(result)
}
