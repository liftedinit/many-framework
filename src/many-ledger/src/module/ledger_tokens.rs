use crate::error;
use crate::migration::tokens::TOKEN_MIGRATION;
use crate::module::account::verify_account_role;
use crate::module::LedgerModuleImpl;
use crate::storage::LedgerStorage;
use many_error::ManyError;
use many_identity::Address;
use many_modules::account;
use many_modules::account::features::TryCreateFeature;
use many_modules::account::Role;
use many_modules::ledger::{
    LedgerTokensModuleBackend, TokenAddExtendedInfoArgs, TokenAddExtendedInfoReturns,
    TokenCreateArgs, TokenCreateReturns, TokenInfoArgs, TokenInfoReturns,
    TokenRemoveExtendedInfoArgs, TokenRemoveExtendedInfoReturns, TokenUpdateArgs,
    TokenUpdateReturns,
};
use many_types::Either;

impl LedgerModuleImpl {
    #[allow(dead_code)]
    /// Used only in cucumber tests
    pub fn token_identity(&self) -> Result<many_identity::Address, ManyError> {
        self.storage.get_token_identity()
    }
}

fn verify_tokens_acl(
    storage: &LedgerStorage,
    sender: &Address,
    addr: &Address,
    roles: impl IntoIterator<Item = Role>,
) -> Result<(), ManyError> {
    if addr != sender {
        if let Some(account) = storage.get_account(addr)? {
            verify_account_role(
                &account,
                sender,
                account::features::tokens::TokenAccountLedger::ID,
                roles,
            )?;
        } else {
            return Err(error::unauthorized());
        }
    }
    Ok(())
}

#[cfg(not(feature = "disable_token_sender_check"))]
fn verify_tokens_sender(sender: &Address, token_identity: Address) -> Result<(), ManyError> {
    if *sender != token_identity {
        return Err(error::invalid_sender());
    }
    Ok(())
}

impl LedgerTokensModuleBackend for LedgerModuleImpl {
    fn create(
        &mut self,
        sender: &Address,
        args: TokenCreateArgs,
    ) -> Result<TokenCreateReturns, ManyError> {
        if !self.storage.migrations().is_active(&TOKEN_MIGRATION) {
            return Err(ManyError::invalid_method_name("tokens.create"));
        }

        #[cfg(not(feature = "disable_token_sender_check"))]
        verify_tokens_sender(sender, self.storage.get_token_identity()?)?;

        if let Some(Either::Left(addr)) = &args.owner {
            verify_tokens_acl(&self.storage, sender, addr, [Role::CanTokensCreate])?;
        }

        let ticker = &args.summary.ticker;
        if self
            .storage
            .get_symbols_and_tickers()?
            .values()
            .any(|v| v == ticker)
        {
            return Err(ManyError::unknown(format!(
                "The ticker {ticker} already exists on this network"
            )));
        }
        self.storage.create_token(sender, args)
    }

    fn info(&self, _sender: &Address, args: TokenInfoArgs) -> Result<TokenInfoReturns, ManyError> {
        // Check the memory symbol cache for requested symbol
        if !self.storage.migrations().is_active(&TOKEN_MIGRATION) {
            return Err(ManyError::invalid_method_name("tokens.info"));
        }

        let symbol = &args.symbol;
        if !self.storage.get_symbols()?.contains(symbol) {
            return Err(ManyError::unknown(format!(
                "The symbol {symbol} was not found"
            )));
        }
        self.storage.info_token(args)
    }

    fn update(
        &mut self,
        sender: &Address,
        args: TokenUpdateArgs,
    ) -> Result<TokenUpdateReturns, ManyError> {
        if !self.storage.migrations().is_active(&TOKEN_MIGRATION) {
            return Err(ManyError::invalid_method_name("tokens.update"));
        }

        // Get the current owner and check if we're allowed to update this token
        let current_owner = self.storage.get_owner(&args.symbol)?;
        match current_owner {
            Some(addr) => {
                verify_tokens_acl(&self.storage, sender, &addr, [Role::CanTokensUpdate])?;
            }
            None => {
                return Err(ManyError::unknown(
                    "Unable to update, this token is immutable",
                ))
            }
        }

        // Check the memory symbol cache for requested symbol
        let symbol = &args.symbol;
        if !self.storage.get_symbols()?.contains(symbol) {
            return Err(ManyError::unknown(format!(
                "The symbol {symbol} was not found"
            )));
        }

        self.storage.update_token(sender, args)
    }

    fn add_extended_info(
        &mut self,
        sender: &Address,
        args: TokenAddExtendedInfoArgs,
    ) -> Result<TokenAddExtendedInfoReturns, ManyError> {
        if !self.storage.migrations().is_active(&TOKEN_MIGRATION) {
            return Err(ManyError::invalid_method_name("tokens.addExtendedInfo"));
        }

        let current_owner = self.storage.get_owner(&args.symbol)?;
        match current_owner {
            Some(addr) => {
                verify_tokens_acl(
                    &self.storage,
                    sender,
                    &addr,
                    [Role::CanTokensAddExtendedInfo],
                )?;
            }
            None => {
                return Err(ManyError::unknown(
                    "Unable to update, this token is immutable",
                ))
            }
        }

        self.storage.add_extended_info(args)
    }

    fn remove_extended_info(
        &mut self,
        sender: &Address,
        args: TokenRemoveExtendedInfoArgs,
    ) -> Result<TokenRemoveExtendedInfoReturns, ManyError> {
        if !self.storage.migrations().is_active(&TOKEN_MIGRATION) {
            return Err(ManyError::invalid_method_name("tokens.removeExtendedInfo"));
        }

        let current_owner = self.storage.get_owner(&args.symbol)?;
        match current_owner {
            Some(addr) => {
                verify_tokens_acl(
                    &self.storage,
                    sender,
                    &addr,
                    [Role::CanTokensRemoveExtendedInfo],
                )?;
            }
            None => {
                return Err(ManyError::unknown(
                    "Unable to update, this token is immutable",
                ))
            }
        }

        self.storage.remove_extended_info(args)
    }
}
