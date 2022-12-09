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

fn verify_tokens_acl(
    storage: &LedgerStorage,
    sender: &Address,
    addr: &Address,
    roles: impl IntoIterator<Item = Role>,
) -> Result<(), ManyError> {
    if addr != sender {
        if let Some(account) = storage.get_account(addr) {
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

impl LedgerTokensModuleBackend for LedgerModuleImpl {
    fn create(
        &mut self,
        sender: &Address,
        args: TokenCreateArgs,
    ) -> Result<TokenCreateReturns, ManyError> {
        if !self.storage.migrations().is_active(&TOKEN_MIGRATION) {
            return Err(ManyError::unknown(
                "Token Migration needs to be active to use this endpoint",
            ));
        }

        // TODO: Limit token creation to given sender
        // | A server implementing this attribute SHOULD protect the endpoints described in this form in some way.
        // | For example, endpoints SHOULD error if the sender isn't from a certain address.

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
            return Err(ManyError::unknown(
                "Token Migration needs to be active to use this endpoint",
            ));
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
        // TODO: Limit token update to given sender
        // | A server implementing this attribute SHOULD protect the endpoints described in this form in some way.
        // | For example, endpoints SHOULD error if the sender isn't from a certain address.

        if !self.storage.migrations().is_active(&TOKEN_MIGRATION) {
            return Err(ManyError::unknown(
                "Token Migration needs to be active to use this endpoint",
            ));
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
        // TODO: Limit adding extended info to given sender
        // | A server implementing this attribute SHOULD protect the endpoints described in this form in some way.
        // | For example, endpoints SHOULD error if the sender isn't from a certain address.
        if !self.storage.migrations().is_active(&TOKEN_MIGRATION) {
            return Err(ManyError::unknown(
                "Token Migration needs to be active to use this endpoint",
            ));
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
        // TODO: Limit adding extended info to given sender
        // | A server implementing this attribute SHOULD protect the endpoints described in this form in some way.
        // | For example, endpoints SHOULD error if the sender isn't from a certain address.
        if !self.storage.migrations().is_active(&TOKEN_MIGRATION) {
            return Err(ManyError::unknown(
                "Token Migration needs to be active to use this endpoint",
            ));
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
