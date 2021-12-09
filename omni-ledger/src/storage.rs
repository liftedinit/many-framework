use crate::error;
use crate::error::unknown_symbol;
use crate::module::balance::SymbolList;
use minicbor::data::Tag;
use minicbor::{encode, Decode, Decoder, Encode, Encoder};
use num_bigint::BigUint;
use omni::{Identity, OmniError};
use sha3::Digest;
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter, Octal};
use std::path::Path;

/// Returns the key for the persistent kv-store.
fn key_for(id: &Identity, symbol: &String) -> Vec<u8> {
    format!("/balances/{}/{}", id.to_string(), symbol).into_bytes()
}

type TokenAmountStorage = num_bigint::BigUint;

#[repr(transparent)]
#[derive(Default, Debug, Hash, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct TokenAmount(TokenAmountStorage);

impl TokenAmount {
    pub fn zero() -> Self {
        Self(0u8.into())
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0u8.into()
    }

    pub(crate) fn hash<H: sha3::Digest>(&self, state: &mut H) {
        state.update("amount\0");
        state.update(self.0.to_bytes_be());
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_bytes_be()
    }
}

impl From<u64> for TokenAmount {
    fn from(v: u64) -> Self {
        TokenAmount(v.into())
    }
}

impl From<u128> for TokenAmount {
    fn from(v: u128) -> Self {
        TokenAmount(v.into())
    }
}

impl From<Vec<u8>> for TokenAmount {
    fn from(v: Vec<u8>) -> Self {
        TokenAmount(num_bigint::BigUint::from_bytes_be(v.as_slice()))
    }
}

impl From<num_bigint::BigUint> for TokenAmount {
    fn from(v: BigUint) -> Self {
        TokenAmount(v)
    }
}

impl Display for TokenAmount {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::ops::AddAssign for TokenAmount {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

impl std::ops::SubAssign for TokenAmount {
    fn sub_assign(&mut self, rhs: Self) {
        if self.0 <= rhs.0 {
            self.0 = TokenAmountStorage::from(0u8);
        } else {
            self.0 -= rhs.0
        }
    }
}

impl Encode for TokenAmount {
    fn encode<W: encode::Write>(&self, e: &mut Encoder<W>) -> Result<(), encode::Error<W::Error>> {
        e.tag(Tag::PosBignum)?.bytes(&self.0.to_bytes_be())?;
        Ok(())
    }
}

impl<'b> Decode<'b> for TokenAmount {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        if d.tag()? != Tag::PosBignum {
            return Err(minicbor::decode::Error::Message("Invalid tag."));
        }

        let bytes = d.bytes()?.to_vec();
        Ok(TokenAmount::from(bytes))
    }
}

#[derive(Debug, Clone)]
pub enum Transaction {
    Mint(Identity, TokenAmount, String),
    Burn(Identity, TokenAmount, String),
    Send(Identity, Identity, TokenAmount, String),
}

impl Transaction {
    pub(crate) fn hash<H: sha3::Digest>(&self, state: &mut H) {
        match self {
            Transaction::Mint(id, amount, symbol) => {
                state.update("mint\0");
                state.update(id.to_vec().as_slice());
                state.update("\0");
                amount.hash(state);
                state.update("symbol\0");
                state.update(symbol.as_bytes());
                state.update("\0");
            }
            Transaction::Burn(id, amount, symbol) => {
                state.update("burn\0");
                state.update(id.to_vec().as_slice());
                state.update("\0");
                amount.hash(state);
                state.update("symbol\0");
                state.update(symbol.as_bytes());
                state.update("\0");
            }
            Transaction::Send(from, to, amount, symbol) => {
                state.update("send\0");
                state.update(from.to_vec().as_slice());
                state.update("\0");
                state.update(to.to_vec().as_slice());
                state.update("\0");
                amount.hash(state);
                state.update("symbol\0");
                state.update(symbol.as_bytes());
                state.update("\0");
            }
        }
    }
}

pub struct LedgerStorage {
    symbols: BTreeSet<String>,
    minters: BTreeSet<Identity>,

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
        let mut persistent_store = fmerk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

        let symbols = persistent_store.get(b"/config/symbols").unwrap().unwrap();
        let minters = persistent_store.get(b"/config/minters").unwrap().unwrap();

        Ok(Self {
            symbols: String::from_utf8(symbols)
                .unwrap()
                .split(":")
                .map(|x| x.to_owned())
                .collect(),
            minters: String::from_utf8(minters)
                .unwrap()
                .split(":")
                .map(|x| Identity::from_str(x))
                .collect::<Result<BTreeSet<Identity>, OmniError>>()
                .map_err(|e| e.to_string())?,
            persistent_store,
            blockchain,
        })
    }

    pub fn new<P: AsRef<Path>>(
        symbols: BTreeSet<String>,
        initial_balances: BTreeMap<Identity, BTreeMap<String, u128>>,
        minters: BTreeSet<Identity>,
        persistent_path: P,
        blockchain: bool,
    ) -> Result<Self, String> {
        let mut persistent_store = fmerk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

        let mut batch: Vec<fmerk::BatchEntry> = Vec::new();
        use itertools::Itertools;

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
            fmerk::Op::Put(minters.iter().map(|i| i.to_string()).join(":").into_bytes()),
        ));
        batch.push((
            b"/config/symbols".to_vec(),
            fmerk::Op::Put(symbols.iter().join(":").into_bytes()),
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

    pub fn commit(&mut self) -> () {
        self.persistent_store.commit(&[]).unwrap();
    }

    pub fn get_balance(&self, identity: &Identity, symbol: &String) -> TokenAmount {
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

    pub fn get_all_balances(&self, identity: &Identity) -> BTreeMap<&String, TokenAmount> {
        if identity.is_anonymous() {
            // Anonymous cannot hold funds.
            BTreeMap::new()
        } else {
            let mut result = BTreeMap::new();
            for symbol in &self.symbols {
                match self.persistent_store.get(&key_for(identity, &symbol)) {
                    Ok(None) => {}
                    Ok(Some(value)) => {
                        result.insert(symbol, TokenAmount::from(value));
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
        symbols: BTreeSet<String>,
    ) -> BTreeMap<&String, TokenAmount> {
        if symbols.is_empty() {
            self.get_all_balances(identity)
        } else {
            self.get_all_balances(identity)
                .into_iter()
                .filter(|(k, _v)| symbols.contains(k.as_str()))
                .collect()
        }
    }

    pub fn mint(
        &mut self,
        to: &Identity,
        symbol: &String,
        amount: TokenAmount,
    ) -> Result<(), OmniError> {
        if amount.is_zero() {
            // NOOP.
            return Ok(());
        }
        if to.is_anonymous() {
            return Err(error::anonymous_cannot_hold_funds());
        }

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
        symbol: &String,
        amount: TokenAmount,
    ) -> Result<(), OmniError> {
        if amount.is_zero() {
            // NOOP.
            return Ok(());
        }
        if to.is_anonymous() {
            return Err(error::anonymous_cannot_hold_funds());
        }

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
        symbol: &String,
        amount: TokenAmount,
    ) -> Result<(), OmniError> {
        if amount.is_zero() {
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
        let mut amount_to = self.get_balance(to, symbol);

        amount_to += amount.clone();
        amount_from -= amount.clone();

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
