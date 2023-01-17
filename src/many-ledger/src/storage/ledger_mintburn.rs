use crate::error;
use crate::storage::ledger_tokens::key_for_symbol;
use crate::storage::{key_for_account_balance, LedgerStorage};
use many_error::ManyError;
use many_modules::ledger::TokenInfoArgs;
use many_types::ledger::{LedgerTokensAddressMap, Symbol, TokenAmount, TokenInfoSupply};
use merk::{BatchEntry, Op};
use std::collections::BTreeSet;

impl LedgerStorage {
    pub(crate) fn get_token_supply(&self, symbol: &Symbol) -> Result<TokenInfoSupply, ManyError> {
        Ok(self
            .info_token(TokenInfoArgs {
                symbol: *symbol,
                extended_info: None,
            })?
            .info
            .supply)
    }

    // TODO: Pass an iterator instead?
    pub fn mint_token(
        &mut self,
        symbol: Symbol,
        distribution: &LedgerTokensAddressMap,
    ) -> Result<(), ManyError> {
        let mut batch: Vec<BatchEntry> = Vec::new();
        let mut circulating = TokenAmount::zero();
        for (address, amount) in distribution.iter() {
            circulating += amount.clone(); // TODO: Remove clone

            // Get current amount, if any
            let balance = self.get_multiple_balances(address, &BTreeSet::from([symbol]))?;
            // TODO: Remove clone
            let new_balance = if balance.contains_key(&symbol) {
                balance[&symbol].clone() + amount.clone()
            } else {
                amount.clone()
            };
            let key = key_for_account_balance(address, &symbol);
            batch.push((key, Op::Put(new_balance.to_vec())));
        }

        // Update circulating supply
        let mut info = self
            .info_token(TokenInfoArgs {
                symbol,
                extended_info: None,
            })?
            .info;
        info.supply.circulating += circulating;

        // Update the supply total if the circulating supply is greater than the current total
        if info.supply.circulating > info.supply.total {
            info.supply.total = info.supply.circulating.clone();
        }

        batch.push((
            key_for_symbol(&symbol).into(),
            Op::Put(minicbor::to_vec(&info).map_err(ManyError::serialization_error)?),
        ));

        self.persistent_store
            .apply(batch.as_slice())
            .map_err(error::storage_apply_failed)?;

        self.maybe_commit()?;

        Ok(())
    }

    // TODO: Pass iterators instead?
    pub fn burn_token(
        &mut self,
        symbol: Symbol,
        distribution: LedgerTokensAddressMap,
        balances: LedgerTokensAddressMap,
    ) -> Result<(), ManyError> {
        let mut batch: Vec<BatchEntry> = Vec::new();
        let mut circulating = TokenAmount::zero();
        for ((d_addr, d_amount), (b_addr, b_amount)) in
            distribution.into_iter().zip(balances.into_iter())
        {
            if d_addr != b_addr {
                return Err(ManyError::unknown(
                    "Distribution address != balance address.",
                )); // TODO: Refactor
            }
            circulating += d_amount.clone(); // TODO: Remove clone

            let new_balance = b_amount - d_amount;
            let key = key_for_account_balance(&d_addr, &symbol);
            batch.push((key, Op::Put(new_balance.to_vec())));
        }

        // Update circulating supply
        let mut info = self
            .info_token(TokenInfoArgs {
                symbol,
                extended_info: None,
            })?
            .info;
        info.supply.circulating -= circulating.clone();
        info.supply.total -= circulating;

        batch.push((
            key_for_symbol(&symbol).into(),
            Op::Put(minicbor::to_vec(&info).map_err(ManyError::serialization_error)?),
        ));

        self.persistent_store
            .apply(batch.as_slice())
            .map_err(error::storage_apply_failed)?;

        self.maybe_commit()?;

        Ok(())
    }
}
