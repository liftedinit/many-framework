use crate::error;
use crate::utils::TokenAmount;
use omni::{Identity, OmniError};
use omni_abci::types::AbciCommitInfo;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::str::FromStr;
use tracing::info;

/// Returns the key for the persistent kv-store.
pub(crate) fn key_for(id: &Identity, symbol: &str) -> Vec<u8> {
    format!("/balances/{}/{}", id.to_string(), symbol).into_bytes()
}

pub struct LedgerStorage {
    symbols: BTreeSet<String>,
    minters: BTreeMap<String, Vec<Identity>>,

    persistent_store: fmerk::Merk,

    /// When this is true, we do not commit every transactions as they come,
    /// but wait for a `commit` call before committing the batch to the
    /// persistent store.
    blockchain: bool,
}

impl std::fmt::Debug for LedgerStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LedgerStorage")
            .field("symbols", &self.symbols)
            .finish()
    }
}

impl LedgerStorage {
    pub fn load<P: AsRef<Path>>(persistent_path: P, blockchain: bool) -> Result<Self, String> {
        let persistent_store = fmerk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

        let symbols = persistent_store
            .get(b"/config/symbols")
            .map_err(|e| e.to_string())?;
        let minters = persistent_store
            .get(b"/config/minters")
            .map_err(|e| e.to_string())?;

        let symbols: BTreeSet<String> = symbols
            .map_or_else(|| Ok(Default::default()), |bytes| minicbor::decode(&bytes))
            .map_err(|e| e.to_string())?;
        let minters = minters
            .map_or_else(|| Ok(Default::default()), |bytes| minicbor::decode(&bytes))
            .map_err(|e| e.to_string())?;

        Ok(Self {
            symbols,
            minters,
            persistent_store,
            blockchain,
        })
    }

    pub fn new<P: AsRef<Path>>(
        symbols: BTreeSet<String>,
        initial_balances: BTreeMap<Identity, BTreeMap<String, u128>>,
        minters: BTreeMap<String, Vec<Identity>>,
        persistent_path: P,
        blockchain: bool,
    ) -> Result<Self, String> {
        let mut persistent_store = fmerk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

        let mut batch: Vec<fmerk::BatchEntry> = Vec::new();

        for (k, v) in initial_balances.into_iter() {
            for (symbol, tokens) in v.into_iter() {
                if !symbols.contains(&symbol) {
                    return Err(format!(r#"Unknown symbol "{}" for identity {}"#, symbol, k));
                }

                let key = key_for(&k, &symbol);
                batch.push((key, fmerk::Op::Put(TokenAmount::from(tokens).to_vec())));
            }
        }

        batch.push((
            b"/config/minters".to_vec(),
            fmerk::Op::Put(minicbor::to_vec(&minters).map_err(|e| e.to_string())?),
        ));
        batch.push((
            b"/config/symbols".to_vec(),
            fmerk::Op::Put(minicbor::to_vec(&symbols).map_err(|e| e.to_string())?),
        ));

        persistent_store
            .apply(batch.as_slice())
            .map_err(|e| e.to_string())?;
        persistent_store.commit(&[]).map_err(|e| e.to_string())?;

        Ok(Self {
            symbols,
            minters,
            persistent_store,
            blockchain,
        })
    }

    pub fn get_symbols(&self) -> Vec<&str> {
        self.symbols.iter().map(|x| x.as_str()).collect()
    }

    pub fn can_mint(&self, id: &Identity, symbol: &str) -> bool {
        self.minters.get(symbol).map_or(false, |x| x.contains(id))
    }

    pub fn get_height(&self) -> u64 {
        self.persistent_store
            .get(b"/height")
            .unwrap()
            .map_or(0u64, |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                u64::from_be_bytes(bytes)
            })
    }

    pub fn commit(&mut self) -> AbciCommitInfo {
        let current_height = self.get_height() + 1;
        self.persistent_store
            .apply(&[(
                b"/height".to_vec(),
                fmerk::Op::Put(current_height.to_be_bytes().to_vec()),
            )])
            .unwrap();
        self.persistent_store.commit(&[]).unwrap();

        AbciCommitInfo {
            retain_height: current_height,
            hash: self.hash(),
        }
    }

    pub fn get_balance(&self, identity: &Identity, symbol: &str) -> TokenAmount {
        if identity.is_anonymous() {
            TokenAmount::zero()
        } else {
            let key = key_for(identity, symbol);
            match self.persistent_store.get(&key).unwrap() {
                None => TokenAmount::zero(),
                Some(amount) => TokenAmount::from(amount),
            }
        }
    }

    fn get_all_balances(&self, identity: &Identity) -> BTreeMap<&str, TokenAmount> {
        if identity.is_anonymous() {
            // Anonymous cannot hold funds.
            BTreeMap::new()
        } else {
            let mut result = BTreeMap::new();
            for symbol in &self.symbols {
                match self.persistent_store.get(&key_for(identity, symbol)) {
                    Ok(None) => {}
                    Ok(Some(value)) => {
                        result.insert(symbol.as_str(), TokenAmount::from(value));
                    }
                    Err(_) => {}
                }
            }

            result
        }
    }

    pub fn get_multiple_balances(
        &self,
        identity: &Identity,
        symbols: &BTreeSet<String>,
    ) -> BTreeMap<&str, TokenAmount> {
        if symbols.is_empty() {
            self.get_all_balances(identity)
        } else {
            self.get_all_balances(identity)
                .into_iter()
                .filter(|(k, _v)| symbols.contains(*k))
                .collect()
        }
    }

    pub fn generate_proof(
        &mut self,
        identity: &Identity,
        symbols: &BTreeSet<String>,
    ) -> Result<Vec<u8>, OmniError> {
        self.persistent_store
            .prove(
                symbols
                    .iter()
                    .map(|s| key_for(identity, s))
                    .collect::<Vec<Vec<u8>>>()
                    .as_slice(),
            )
            .map_err(|e| OmniError::unknown(e.to_string()))
    }

    pub fn mint(
        &mut self,
        to: &Identity,
        symbol: &str,
        amount: TokenAmount,
    ) -> Result<(), OmniError> {
        if amount.is_zero() {
            // NOOP.
            return Ok(());
        }
        if to.is_anonymous() {
            return Err(error::anonymous_cannot_hold_funds());
        }

        info!("mint({}, {} {})", to, &amount, symbol);

        let mut balance = self.get_balance(to, symbol);
        balance += amount;

        self.persistent_store
            .apply(&[(key_for(to, symbol), fmerk::Op::Put(balance.to_vec()))])
            .unwrap();

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }
        Ok(())
    }

    pub fn burn(
        &mut self,
        to: &Identity,
        symbol: &str,
        amount: TokenAmount,
    ) -> Result<(), OmniError> {
        if amount.is_zero() {
            // NOOP.
            return Ok(());
        }
        if to.is_anonymous() {
            return Err(error::anonymous_cannot_hold_funds());
        }

        info!("burn({}, {} {})", to, &amount, symbol);

        let mut balance = self.get_balance(to, symbol);
        balance -= amount;

        self.persistent_store
            .apply(&[(key_for(to, symbol), fmerk::Op::Put(balance.to_vec()))])
            .unwrap();

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }

        Ok(())
    }

    pub fn send(
        &mut self,
        from: &Identity,
        to: &Identity,
        symbol: &str,
        amount: TokenAmount,
    ) -> Result<(), OmniError> {
        if amount.is_zero() || from == to {
            // NOOP.
            return Ok(());
        }
        if to.is_anonymous() || from.is_anonymous() {
            return Err(error::anonymous_cannot_hold_funds());
        }

        let mut amount_from = self.get_balance(from, symbol);
        if amount > amount_from {
            return Err(error::insufficient_funds());
        }

        info!("send({} => {}, {} {})", from, to, &amount, symbol);

        let mut amount_to = self.get_balance(to, symbol);
        amount_to += amount.clone();
        amount_from -= amount;

        self.persistent_store
            .apply(&[
                (key_for(from, symbol), fmerk::Op::Put(amount_from.to_vec())),
                (key_for(to, symbol), fmerk::Op::Put(amount_to.to_vec())),
            ])
            .unwrap();

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }

        Ok(())
    }

    pub fn hash(&self) -> Vec<u8> {
        self.persistent_store.root_hash().to_vec()
    }
}
