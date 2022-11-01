use crate::error;
use crate::module::account::verify_account_role;
use crate::module::LedgerModuleImpl;
use many_error::ManyError;
use many_identity::Address;
use many_modules::account::Role;
use many_modules::{ledger, EmptyReturn};

impl ledger::LedgerCommandsModuleBackend for LedgerModuleImpl<'_> {
    fn send(&mut self, sender: &Address, args: ledger::SendArgs) -> Result<EmptyReturn, ManyError> {
        let ledger::SendArgs {
            from,
            to,
            amount,
            symbol,
        } = args;

        let from = from.as_ref().unwrap_or(sender);
        if from != sender {
            if let Some(account) = self.storage.get_account(from) {
                verify_account_role(&account, sender, [Role::CanLedgerTransact])?;
            } else {
                return Err(error::unauthorized());
            }
        }

        self.storage.send(from, &to, &symbol, amount)?;
        Ok(EmptyReturn)
    }
}
