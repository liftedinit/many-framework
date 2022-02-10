use crate::storage::key_for_account;
use omni::types::ledger::{Symbol, TokenAmount};
use omni::Identity;
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
    let keys: Vec<Vec<u8>> = symbols
        .iter()
        .map(|s| key_for_account(identity, s))
        .collect();
    let values =
        fmerk::verify_proof(bytes, keys.as_slice(), *expected_hash).map_err(|e| e.to_string())?;

    let mut result = BTreeMap::new();
    for (symbol, amount) in symbols.iter().zip(values.iter()) {
        result.insert(
            symbol.clone(),
            amount
                .as_ref()
                .map_or(TokenAmount::zero(), |x| TokenAmount::from(x.clone())),
        );
    }

    Ok(result)
}
