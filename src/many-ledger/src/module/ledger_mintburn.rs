use crate::error;
use crate::migration::tokens::TOKEN_MIGRATION;
use crate::module::LedgerModuleImpl;
use many_error::ManyError;
use many_identity::Address;
use many_modules::events::EventInfo;
use many_modules::ledger;
use many_modules::ledger::{TokenBurnArgs, TokenBurnReturns, TokenMintArgs, TokenMintReturns};
use many_types::ledger::{LedgerTokensAddressMap, Symbol, TokenAmount};
use std::collections::BTreeSet;

/// Check if a symbol exists in the storage
fn check_symbol_exists(symbol: &Symbol, symbols: BTreeSet<Symbol>) -> Result<(), ManyError> {
    if !symbols.contains(symbol) {
        return Err(error::symbol_not_found(symbol.to_string()));
    }
    Ok(())
}

impl ledger::LedgerMintBurnModuleBackend for LedgerModuleImpl {
    fn mint(
        &mut self,
        sender: &Address,
        args: TokenMintArgs,
    ) -> Result<TokenMintReturns, ManyError> {
        if !self.storage.migrations().is_active(&TOKEN_MIGRATION) {
            return Err(ManyError::invalid_method_name("tokens.mint"));
        }

        let TokenMintArgs {
            symbol,
            distribution,
            memo,
        } = args;
        // Only the token identity is able to mint tokens
        self.storage.verify_tokens_sender(sender)?;

        check_symbol_exists(&symbol, self.storage.get_symbols()?)?;

        // Get current token supply
        let current_supply = self.storage.get_token_supply(&symbol)?;

        // Check we don't bust the current max and that distribution doesn't contain zero
        let amount_to_mint =
            distribution
                .iter()
                .try_fold(TokenAmount::zero(), |acc, (addr, x)| {
                    if x > &TokenAmount::zero() {
                        Ok(acc + x.clone())
                    } else {
                        Err(error::unable_to_distribute_zero(addr))
                    }
                })?; // TODO: Remove clone
        if let Some(maximum) = current_supply.maximum {
            let new_amount = amount_to_mint + current_supply.circulating;
            if new_amount > maximum {
                return Err(error::over_maximum_supply(symbol, new_amount, maximum));
            }
        }

        // Mint into storage
        self.storage.mint_token(symbol, &distribution)?;

        // Log event
        self.storage.log_event(EventInfo::TokenMint {
            symbol,
            distribution,
            memo,
        })?;

        Ok(TokenMintReturns {})
    }

    fn burn(
        &mut self,
        sender: &Address,
        args: TokenBurnArgs,
    ) -> Result<TokenBurnReturns, ManyError> {
        if !self.storage.migrations().is_active(&TOKEN_MIGRATION) {
            return Err(ManyError::invalid_method_name("tokens.burn"));
        }

        let TokenBurnArgs {
            symbol,
            distribution,
            memo,
            error_on_under_burn,
        } = args;
        // Only the token identity is able to burn tokens
        self.storage.verify_tokens_sender(sender)?;

        check_symbol_exists(&symbol, self.storage.get_symbols()?)?;

        // Disable partial burn, for now
        if let Some(error) = error_on_under_burn {
            if !error {
                return Err(error::partial_burn_disabled());
            }
        }

        // Verify balance
        let mut balances = LedgerTokensAddressMap::new();
        for (address, amount) in distribution.iter() {
            if amount.is_zero() {
                return Err(error::unable_to_distribute_zero(address));
            }

            let balance = self
                .storage
                .get_multiple_balances(address, &BTreeSet::from_iter([symbol]))?;
            if !(balance.contains_key(&symbol) && &balance[&symbol] >= amount) {
                return Err(error::missing_funds(
                    symbol,
                    amount,
                    balance.get(&symbol).unwrap_or(&TokenAmount::zero()),
                ));
            }
            balances.insert(*address, balance[&symbol].clone());
        }

        // Burn from storage
        self.storage
            .burn_token(symbol, distribution.clone(), balances)?;

        // Log event
        self.storage.log_event(EventInfo::TokenBurn {
            symbol,
            distribution: distribution.clone(),
            memo,
        })?;

        Ok(TokenBurnReturns { distribution })
    }
}
