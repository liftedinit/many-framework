use crate::storage::{key_for_account_balance, LedgerStorage};
use many_error::ManyError;
use many_identity::Address;
use many_types::ledger::{Symbol, TokenAmount};
use std::collections::{BTreeMap, BTreeSet};

impl LedgerStorage {
    fn get_all_balances(
        &self,
        identity: &Address,
    ) -> Result<BTreeMap<Symbol, TokenAmount>, ManyError> {
        if identity.is_anonymous() {
            // Anonymous cannot hold funds.
            Ok(BTreeMap::new())
        } else {
            let mut result = BTreeMap::new();
            for symbol in self.get_symbols()? {
                match self
                    .persistent_store
                    .get(&key_for_account_balance(identity, &symbol))
                {
                    Ok(None) => {}
                    Ok(Some(value)) => {
                        result.insert(symbol, TokenAmount::from(value));
                    }
                    Err(_) => {}
                }
            }

            Ok(result)
        }
    }

    pub fn get_multiple_balances(
        &self,
        identity: &Address,
        symbols: &BTreeSet<Symbol>,
    ) -> Result<BTreeMap<Symbol, TokenAmount>, ManyError> {
        if symbols.is_empty() {
            Ok(self.get_all_balances(identity)?)
        } else {
            Ok(self
                .get_all_balances(identity)?
                .into_iter()
                .filter(|(k, _v)| symbols.contains(k))
                .collect())
        }
    }
}
