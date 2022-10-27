use crate::storage::{key_for_account_balance, LedgerStorage};
use many_identity::Address;
use many_types::ledger::{Symbol, TokenAmount};

pub const MIGRATIONS_KEY: &[u8] = b"/config/migrations";

impl LedgerStorage {
    pub fn get_balance(&self, identity: &Address, symbol: &Symbol) -> TokenAmount {
        if identity.is_anonymous() {
            TokenAmount::zero()
        } else {
            let key = key_for_account_balance(identity, symbol);
            match self.persistent_store.get(&key).unwrap() {
                None => TokenAmount::zero(),
                Some(amount) => TokenAmount::from(amount),
            }
        }
    }
}
