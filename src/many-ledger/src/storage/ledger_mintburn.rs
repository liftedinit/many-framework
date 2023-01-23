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

    pub fn mint_token(
        &mut self,
        symbol: Symbol,
        distribution: &LedgerTokensAddressMap,
    ) -> Result<(), ManyError> {
        let mut batch: Vec<BatchEntry> = Vec::new();
        let mut circulating = TokenAmount::zero();
        for (address, amount) in distribution.iter() {
            circulating += amount;
            let new_balance = self
                .get_multiple_balances(address, &BTreeSet::from([symbol]))?
                .get(&symbol)
                .map_or(amount.clone(), |b| b + amount);
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
        info.supply.total = info.supply.circulating.clone();

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

    pub fn burn_token(
        &mut self,
        symbol: Symbol,
        distribution: &LedgerTokensAddressMap,
        balances: LedgerTokensAddressMap,
    ) -> Result<(), ManyError> {
        let mut batch: Vec<BatchEntry> = Vec::new();
        let mut circulating = TokenAmount::zero();
        for ((d_addr, d_amount), (_, ref b_amount)) in distribution.iter().zip(balances.into_iter())
        {
            circulating += d_amount;
            let new_balance = b_amount - d_amount;
            let key = key_for_account_balance(d_addr, &symbol);
            batch.push((key, Op::Put(new_balance.to_vec())));
        }

        // Update circulating supply
        let mut info = self
            .info_token(TokenInfoArgs {
                symbol,
                extended_info: None,
            })?
            .info;
        info.supply.circulating -= &circulating;
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
