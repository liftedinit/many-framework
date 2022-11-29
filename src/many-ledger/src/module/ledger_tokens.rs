use crate::module::LedgerModuleImpl;
use many_error::ManyError;
use many_identity::Address;
use many_modules::ledger::{
    LedgerTokensModuleBackend, TokenAddExtendedInfoArgs, TokenAddExtendedInfoReturns,
    TokenCreateArgs, TokenCreateReturns, TokenInfoArgs, TokenInfoReturns,
    TokenRemoveExtendedInfoArgs, TokenRemoveExtendedInfoReturns, TokenUpdateArgs,
    TokenUpdateReturns,
};

impl LedgerTokensModuleBackend for LedgerModuleImpl {
    fn create(
        &mut self,
        sender: &Address,
        args: TokenCreateArgs,
    ) -> Result<TokenCreateReturns, ManyError> {
        // TODO: Limit token creation to given sender
        // | A server implementing this attribute SHOULD protect the endpoints described in this form in some way.
        // | For example, endpoints SHOULD error if the sender isn't from a certain address.

        let ticker = &args.summary.ticker;
        if self.storage.get_symbols().values().any(|v| v == ticker) {
            return Err(ManyError::unknown(
                "The ticker {ticker} already exists on this network",
            ));
        }
        self.storage.create_token(sender, args)
    }

    fn info(&self, _sender: &Address, args: TokenInfoArgs) -> Result<TokenInfoReturns, ManyError> {
        // Check the memory symbol cache for requested symbol
        let symbol = &args.symbol;
        if !self.storage.get_symbols().contains_key(symbol) {
            return Err(ManyError::unknown("The symbol {symbol} was not found"));
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

        // Check the memory symbol cache for requested symbol
        let symbol = &args.symbol;
        if !self.storage.get_symbols().contains_key(symbol) {
            return Err(ManyError::unknown("The symbol {symbol} was not found"));
        }

        self.storage.update_token(sender, args)
    }

    fn add_extended_info(
        &mut self,
        _sender: &Address,
        args: TokenAddExtendedInfoArgs,
    ) -> Result<TokenAddExtendedInfoReturns, ManyError> {
        // TODO: Limit adding extended info to given sender
        // | A server implementing this attribute SHOULD protect the endpoints described in this form in some way.
        // | For example, endpoints SHOULD error if the sender isn't from a certain address.

        self.storage.add_extended_info(args)
    }

    fn remove_extended_info(
        &mut self,
        _sender: &Address,
        args: TokenRemoveExtendedInfoArgs,
    ) -> Result<TokenRemoveExtendedInfoReturns, ManyError> {
        // TODO: Limit adding extended info to given sender
        // | A server implementing this attribute SHOULD protect the endpoints described in this form in some way.
        // | For example, endpoints SHOULD error if the sender isn't from a certain address.

        self.storage.remove_extended_info(args)
    }
}
