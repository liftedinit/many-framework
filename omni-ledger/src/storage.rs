use crate::error;
use crate::error::unknown_symbol;
use crate::module::balance::SymbolList;
use minicbor::data::Tag;
use minicbor::{encode, Decode, Decoder, Encode, Encoder};
use omni::{Identity, OmniError};
use sha3::Digest;
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter, Octal};

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

impl From<TokenAmountStorage> for TokenAmount {
    fn from(s: TokenAmountStorage) -> Self {
        TokenAmount(s)
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

        Ok(TokenAmount(TokenAmountStorage::from_bytes_be(d.bytes()?)))
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
    accounts: BTreeMap<Identity, BTreeMap<usize, TokenAmount>>,
    symbols: Vec<String>,

    hash_cache: Cell<Option<Vec<u8>>>,
}

impl std::fmt::Debug for LedgerStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LedgerStorage")
            .field("accounts", &self.accounts)
            .finish()
    }
}

impl LedgerStorage {
    pub fn new(
        mut symbols: BTreeSet<String>,
        initial_balances: BTreeMap<Identity, BTreeMap<String, u128>>,
    ) -> Self {
        let mut symbols: Vec<String> = symbols.into_iter().collect();
        let mut accounts = BTreeMap::new();

        for (k, v) in initial_balances.into_iter() {
            let account: &mut BTreeMap<usize, TokenAmount> = accounts.entry(k).or_default();

            for (symbol, tokens) in v.into_iter() {
                match symbols.binary_search(&symbol) {
                    Err(_) => continue,
                    Ok(index) => {
                        account.insert(index, TokenAmount::from(tokens));
                    }
                }
            }
        }

        Self {
            symbols,
            accounts,
            hash_cache: Default::default(),
        }
    }

    pub fn commit(&mut self) -> () {
        self.hash_cache.take();
    }

    pub fn get_balance(&self, identity: &Identity, symbol: &String) -> TokenAmount {
        if identity.is_anonymous() {
            TokenAmount::zero()
        } else {
            if let Some(account) = self.accounts.get(identity) {
                match self.symbols.binary_search(symbol) {
                    Err(_) => TokenAmount::zero(),
                    Ok(ref symbol) => account.get(symbol).cloned().unwrap_or(TokenAmount::zero()),
                }
            } else {
                TokenAmount::zero()
            }
        }
    }

    pub fn get_all_balances(&self, identity: &Identity) -> BTreeMap<&String, &TokenAmount> {
        if identity.is_anonymous() {
            // Anonymous cannot hold funds.
            BTreeMap::new()
        } else {
            match self.accounts.get(identity) {
                None => BTreeMap::new(),
                Some(account) => account
                    .iter()
                    .map(|(k, v)| (&self.symbols[*k], v))
                    .collect(),
            }
        }
    }

    pub fn get_multiple_balances(
        &self,
        identity: &Identity,
        symbols: BTreeSet<String>,
    ) -> BTreeMap<&String, &TokenAmount> {
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

        let index = match self.symbols.binary_search(symbol) {
            Ok(i) => i,
            Err(_) => return Err(unknown_symbol(symbol.clone())),
        };

        *self
            .accounts
            .entry(to.clone())
            .or_default()
            .entry(index)
            .or_default() += amount.clone();

        self.hash_cache.take();
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

        let index = match self.symbols.binary_search(symbol) {
            Ok(i) => i,
            Err(_) => return Err(unknown_symbol(symbol.clone())),
        };

        *self
            .accounts
            .entry(to.clone())
            .or_default()
            .entry(index)
            .or_default() -= amount.clone();

        self.hash_cache.take();
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

        let amount_from = self.get_balance(from, symbol);
        if amount > amount_from {
            return Err(error::insufficient_funds());
        }
        let index = match self.symbols.binary_search(symbol) {
            Ok(i) => i,
            Err(_) => return Err(unknown_symbol(symbol.to_string())),
        };

        *self
            .accounts
            .entry(*from)
            .or_default()
            .entry(index)
            .or_default() -= amount.clone();
        *self
            .accounts
            .entry(*to)
            .or_default()
            .entry(index)
            .or_default() += amount.clone();

        self.hash_cache.take();
        Ok(())
    }

    pub fn hash(&self) -> Vec<u8> {
        let cache = self.hash_cache.as_ptr();

        if let Some(cache) = unsafe { &*cache } {
            cache.clone()
        } else {
            let mut hasher = sha3::Sha3_512::default();
            self.hash_inner(&mut hasher);
            let hash = hasher.finalize().to_vec();

            self.hash_cache.set(Some(hash));
            self.hash()
        }
    }

    fn hash_inner<H: sha3::Digest>(&self, state: &mut H) {
        state.update(b"accounts\0");
        for (k, accounts) in &self.accounts {
            state.update(b"a\0");
            state.update(&k.to_vec());
            state.update(b"t\0");

            for (symbol, amount) in accounts {
                state.update(symbol.to_be_bytes());
                state.update(b"\0");
                amount.hash(state);
            }
        }
    }
}
