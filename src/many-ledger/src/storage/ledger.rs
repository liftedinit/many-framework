use crate::error;
use crate::storage::{key_for_account_balance, LedgerStorage};
use many_error::ManyError;
use many_identity::Address;
use many_protocol::context::Context;
use many_types::ledger::{Symbol, TokenAmount};
use merk::{proofs::Query, BatchEntry, Op};
use std::collections::{BTreeMap, BTreeSet};

impl LedgerStorage {
    pub fn with_balances(
        mut self,
        symbols: &BTreeMap<Symbol, String>,
        initial_balances: &BTreeMap<Address, BTreeMap<Symbol, TokenAmount>>,
    ) -> Result<Self, ManyError> {
        let mut batch: Vec<BatchEntry> = Vec::new();
        for (k, v) in initial_balances.iter() {
            for (symbol, tokens) in v.iter() {
                if !symbols.contains_key(symbol) {
                    return Err(ManyError::unknown(format!(
                        r#"Unknown symbol "{symbol}" for identity {k}"#
                    ))); // TODO: Custom error
                }

                let key = key_for_account_balance(k, symbol);
                batch.push((key, Op::Put(tokens.to_vec())));
            }
        }

        self.persistent_store
            .apply(batch.as_slice())
            .map_err(error::storage_apply_failed)?;

        Ok(self)
    }

    fn get_all_balances(
        &self,
        identity: &Address,
        context: impl AsRef<Context>,
    ) -> Result<BTreeMap<Symbol, TokenAmount>, ManyError> {
        Ok(if identity.is_anonymous() {
            // Anonymous cannot hold funds.
            BTreeMap::new()
        } else {
            let mut result = BTreeMap::new();
            for symbol in self.get_symbols()? {
                match context.as_ref().prove(|| {
                    self.persistent_store
                        .prove({
                            let mut query = Query::new();
                            query.insert_key(key_for_account_balance(identity, &symbol));
                            query
                        })
                        .map_err(|error| ManyError::unknown(error.to_string()))
                }) {
                    Some(error) => Err(ManyError::unknown(error.to_string())),
                    None => Ok(self
                        .persistent_store
                        .get(&key_for_account_balance(identity, &symbol))
                        .map_err(error::storage_get_failed)?
                        .map(|value| result.insert(symbol, TokenAmount::from(value)))
                        .map(|_| ())
                        .unwrap_or_default()),
                }?
            }

            result
        })
    }

    pub fn get_multiple_balances(
        &self,
        identity: &Address,
        symbols: &BTreeSet<Symbol>,
        context: impl AsRef<Context>,
    ) -> Result<BTreeMap<Symbol, TokenAmount>, ManyError> {
        Ok(if symbols.is_empty() {
            self.get_all_balances(identity, context)?
        } else {
            self.get_all_balances(identity, context)?
                .into_iter()
                .filter(|(k, _v)| symbols.contains(k))
                .collect()
        })
    }
}
