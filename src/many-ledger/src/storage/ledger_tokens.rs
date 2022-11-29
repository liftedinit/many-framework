use crate::storage::{key_for_account_balance, LedgerStorage};
use many_error::ManyError;
use many_identity::Address;
use many_modules::events::EventInfo;
use many_modules::ledger::extended_info::{ExtendedInfoKey, TokenExtendedInfo};
use many_modules::ledger::{
    TokenAddExtendedInfoArgs, TokenAddExtendedInfoReturns, TokenCreateArgs, TokenCreateReturns,
    TokenInfoArgs, TokenInfoReturns, TokenRemoveExtendedInfoArgs, TokenRemoveExtendedInfoReturns,
    TokenUpdateArgs, TokenUpdateReturns,
};
use many_types::ledger::{Symbol, TokenAmount, TokenInfo, TokenInfoSupply};
use many_types::AttributeRelatedIndex;
use merk::{BatchEntry, Op};

// TODO: How?
fn generate_new_token_symbol() -> Symbol {
    Symbol::anonymous()
}

pub fn key_for_symbol(symbol: &Symbol) -> Vec<u8> {
    format!("/config/symbols/{symbol}").into_bytes()
}

pub fn key_for_ext_info(symbol: &Symbol) -> Vec<u8> {
    format!("/config/ext_info/{symbol}").into_bytes()
}

impl LedgerStorage {
    pub fn create_token(
        &mut self,
        sender: &Address,
        args: TokenCreateArgs,
    ) -> Result<TokenCreateReturns, ManyError> {
        let TokenCreateArgs {
            summary,
            owner,
            initial_distribution,
            maximum_supply,
            extended_info,
        } = args;

        // Create a new token symbol and store in memory and in the persistent store
        let symbol = generate_new_token_symbol();

        // TODO: I don't like having a memory cache of this kind. Things should be atomic...
        //       We get an inconsistent state if persistent storage ops fail.
        self.symbols.insert(symbol, summary.ticker.clone());
        self.persistent_store
            .apply(&[(
                b"/config/symbols".to_vec(),
                Op::Put(minicbor::to_vec(&self.symbols).map_err(ManyError::serialization_error)?),
            )])
            .map_err(ManyError::unknown)?; // TODO: Custom error

        // Initialize the total supply following the initial token distribution, if any
        let mut batch: Vec<BatchEntry> = Vec::new();
        let total_supply = if let Some(ref initial_distribution) = initial_distribution {
            let mut total_supply = TokenAmount::zero();
            for (k, v) in initial_distribution {
                let key = key_for_account_balance(k, &symbol);
                batch.push((key, Op::Put(v.to_vec())));
                total_supply += v.clone();
            }
            total_supply
        } else {
            TokenAmount::zero()
        };

        let supply = TokenInfoSupply {
            total: total_supply.clone(),
            circulating: total_supply,
            maximum: maximum_supply.clone(),
        };

        // Create the token information and store it in the persistent storage
        let owner = owner.map_or(Some(*sender), |owner| owner);
        let info = TokenInfo {
            symbol,
            summary: summary.clone(),
            supply,
            owner,
        };
        batch.push((
            key_for_symbol(&symbol),
            Op::Put(minicbor::to_vec(&info).map_err(ManyError::serialization_error)?),
        ));

        let ext_info = extended_info
            .clone()
            .map_or(TokenExtendedInfo::default(), |e| e);
        self.persistent_store
            .apply(&[(
                key_for_ext_info(&symbol),
                Op::Put(minicbor::to_vec(&ext_info).map_err(ManyError::serialization_error)?),
            )])
            .map_err(ManyError::unknown)?; // TODO: Custom error

        self.log_event(EventInfo::TokenCreate {
            summary,
            symbol,
            owner,
            initial_distribution,
            maximum_supply,
            extended_info,
        });

        self.persistent_store
            .apply(batch.as_slice())
            .map_err(ManyError::unknown)?; // TODO: Custom

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }

        Ok(TokenCreateReturns { info })
    }

    pub fn info_token(&self, args: TokenInfoArgs) -> Result<TokenInfoReturns, ManyError> {
        let TokenInfoArgs {
            symbol,
            extended_info,
        } = args;

        // Try fetching the token info from the persistent storage
        let token_info_enc = self
            .persistent_store
            .get(&key_for_symbol(&symbol))
            .map_err(ManyError::unknown)?
            .ok_or_else(|| {
                ManyError::unknown(
                    "Symbol {symbol} token information not found in persistent storage",
                )
            })?; // TODO: Custom error

        let ext_info_enc = self
            .persistent_store
            .get(&key_for_ext_info(&symbol))
            .map_err(ManyError::unknown)? // TODO: Custom error
            .ok_or_else(|| {
                ManyError::unknown("Unable to fetch extended info for symbol {symbol}")
            })?; // TODO: Custom error

        let mut ext_info: TokenExtendedInfo =
            minicbor::decode(&ext_info_enc).map_err(ManyError::deserialization_error)?;

        let ext_info = if let Some(indices) = extended_info {
            ext_info.retain(indices)?;
            ext_info
        } else {
            ext_info
        };

        let info: TokenInfo =
            minicbor::decode(&token_info_enc).map_err(ManyError::deserialization_error)?;

        Ok(TokenInfoReturns {
            info,
            extended_info: ext_info,
        })
    }

    pub fn update_token(
        &mut self,
        _sender: &Address,
        args: TokenUpdateArgs,
    ) -> Result<TokenUpdateReturns, ManyError> {
        let TokenUpdateArgs {
            symbol,
            name,
            ticker,
            decimals,
            owner,
            memo,
        } = args;

        // Try fetching the token info from the persistent storage
        if let Some(enc) = self
            .persistent_store
            .get(&key_for_symbol(&symbol))
            .map_err(ManyError::unknown)?
        {
            let mut info: TokenInfo = minicbor::decode(&enc).unwrap();

            // TODO: Check if we can simplify this
            if let Some(name) = name.as_ref() {
                info.summary.name = name.clone();
            }
            if let Some(ticker) = ticker.as_ref() {
                info.summary.ticker = ticker.clone();
            }
            if let Some(decimals) = decimals {
                info.summary.decimals = decimals;
            }
            match owner {
                None => {}
                Some(None) => info.owner = None,
                Some(x) => info.owner = x,
            };

            self.persistent_store
                .apply(&[(
                    key_for_symbol(&symbol),
                    Op::Put(minicbor::to_vec(&info).map_err(ManyError::serialization_error)?),
                )])
                .map_err(ManyError::unknown)?; // TODO: Custom error

            self.log_event(EventInfo::TokenUpdate {
                symbol,
                name,
                ticker,
                decimals,
                owner,
                memo,
            });

            if !self.blockchain {
                self.persistent_store.commit(&[]).unwrap();
            }
        } else {
            return Err(ManyError::unknown(
                "Symbol {symbol} not found in persistent storage",
            ));
        }
        Ok(TokenUpdateReturns {})
    }

    pub fn add_extended_info(
        &mut self,
        args: TokenAddExtendedInfoArgs,
    ) -> Result<TokenAddExtendedInfoReturns, ManyError> {
        let TokenAddExtendedInfoArgs {
            symbol,
            extended_info,
        } = args;

        // Fetch existing extended info, if any
        let mut ext_info = if let Some(ext_info_enc) = self
            .persistent_store
            .get(&key_for_ext_info(&symbol))
            .map_err(ManyError::unknown)?
        // TODO: Custom error
        {
            minicbor::decode(&ext_info_enc).map_err(ManyError::deserialization_error)?
        } else {
            TokenExtendedInfo::new()
        };

        let mut indices = vec![];
        if let Some(memo) = extended_info.memo() {
            ext_info = ext_info.with_memo(memo.clone())?;
            indices.push(AttributeRelatedIndex::from(ExtendedInfoKey::Memo));
        }
        if let Some(logos) = extended_info.visual_logo() {
            ext_info = ext_info.with_visual_logo(logos.clone())?;
            indices.push(AttributeRelatedIndex::from(ExtendedInfoKey::VisualLogo));
        }

        self.persistent_store
            .apply(&[(
                key_for_ext_info(&symbol),
                Op::Put(minicbor::to_vec(&ext_info).map_err(ManyError::serialization_error)?),
            )])
            .map_err(ManyError::unknown)?; // TODO: Custom error

        self.log_event(EventInfo::TokenAddExtendedInfo {
            symbol,
            extended_info: indices,
        });

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }

        Ok(TokenAddExtendedInfoReturns {})
    }

    pub fn remove_extended_info(
        &mut self,
        args: TokenRemoveExtendedInfoArgs,
    ) -> Result<TokenRemoveExtendedInfoReturns, ManyError> {
        let TokenRemoveExtendedInfoArgs {
            symbol,
            extended_info,
        } = args;

        // Fetch existing extended info, if any
        let ext_info_enc = self
            .persistent_store
            .get(&key_for_ext_info(&symbol))
            .map_err(ManyError::unknown)? // TODO: Custom error
            .ok_or_else(|| ManyError::unknown("Unable to remove non-existing extended info"))?; // TODO: Custom error

        let mut ext_info: TokenExtendedInfo =
            minicbor::decode(&ext_info_enc).map_err(ManyError::deserialization_error)?;

        for index in &extended_info {
            if ext_info.contains_index(index)? {
                ext_info.remove(index)?;
            }
        }

        self.persistent_store
            .apply(&[(
                key_for_ext_info(&symbol),
                Op::Put(minicbor::to_vec(&ext_info).map_err(ManyError::serialization_error)?),
            )])
            .map_err(ManyError::unknown)?; // TODO: Custom error

        self.log_event(EventInfo::TokenRemoveExtendedInfo {
            symbol,
            extended_info,
        });

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }

        Ok(TokenRemoveExtendedInfoReturns {})
    }
}
