use crate::balance::SymbolList;
use crate::error;
use minicbor::data::Tag;
use minicbor::{encode, Decode, Decoder, Encode, Encoder};
use omni::{Identity, OmniError};
use sha3::Digest;
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

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
    pub accounts: BTreeMap<Identity, BTreeMap<String, TokenAmount>>,
    pub history: BTreeMap<u64, Vec<Transaction>>,
    pub height: u64,

    hash_cache: Cell<Option<Vec<u8>>>,
}

impl Default for LedgerStorage {
    fn default() -> Self {
        Self {
            accounts: Default::default(),
            history: Default::default(),
            height: 0,
            hash_cache: Default::default(),
        }
    }
}

impl std::fmt::Debug for LedgerStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LedgerStorage")
            .field("accounts", &self.accounts)
            .field("history", &self.history)
            .field("height", &self.height)
            .finish()
    }
}

impl LedgerStorage {
    pub fn commit(&mut self) -> () {
        self.hash_cache.take();
        self.height += 1;
        eprintln!("height: {}", self.height);
    }

    pub fn add_transaction(&mut self, tx: Transaction) {
        self.hash_cache.take();
        self.history.entry(self.height).or_default().push(tx);
    }

    pub fn get_balance(&self, identity: &Identity, symbol: &str) -> TokenAmount {
        if identity.is_anonymous() {
            TokenAmount::zero()
        } else {
            if let Some(account) = self.accounts.get(identity) {
                account.get(symbol).cloned().unwrap_or(TokenAmount::zero())
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
                Some(account) => account.iter().collect(),
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

    pub fn get_transactions_at(&self, index: u64) -> &[Transaction] {
        if let Some(i) = self.history.get(&index) {
            i.as_slice()
        } else {
            &[]
        }
    }

    pub fn transactions_for(&self, account: &Identity) -> Vec<&Transaction> {
        // Number of commits is probably a good enough metric for capacity.
        let mut result = Vec::with_capacity(self.history.len() / 2 + 1);
        for (_height, txs) in &self.history {
            for tx in txs {
                match tx {
                    x @ Transaction::Mint(d, _, _) if d == account => {
                        result.push(x);
                    }
                    x @ Transaction::Burn(d, _, _) if d == account => {
                        result.push(x);
                    }
                    x @ Transaction::Send(f, _, _, _) if f == account => {
                        result.push(x);
                    }
                    x @ Transaction::Send(_, t, _, _) if t == account => {
                        result.push(x);
                    }
                    _ => {}
                }
            }
        }

        result
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

        *self
            .accounts
            .entry(to.clone())
            .or_default()
            .entry(symbol.to_string())
            .or_default() += amount.clone();

        self.add_transaction(Transaction::Mint(to.clone(), amount, symbol.to_string()));
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

        *self
            .accounts
            .entry(to.clone())
            .or_default()
            .entry(symbol.to_string())
            .or_default() -= amount.clone();

        self.add_transaction(Transaction::Burn(
            to.clone(),
            amount.clone(),
            symbol.to_string(),
        ));
        Ok(())
    }

    pub fn send(
        &mut self,
        from: &Identity,
        to: &Identity,
        symbol: &str,
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

        *self
            .accounts
            .entry(*from)
            .or_default()
            .entry(symbol.to_string())
            .or_default() -= amount.clone();
        *self
            .accounts
            .entry(*to)
            .or_default()
            .entry(symbol.to_string())
            .or_default() += amount.clone();

        self.add_transaction(Transaction::Send(
            from.clone(),
            to.clone(),
            amount,
            symbol.to_string(),
        ));

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
        state.update(b"height\0");
        state.update(self.height.to_be_bytes());
        state.update(b"accounts\0");
        for (k, accounts) in &self.accounts {
            state.update(b"a\0");
            state.update(&k.to_vec());
            state.update(b"t\0");

            for (symbol, amount) in accounts {
                state.update(symbol.as_bytes());
                state.update(b"\0");
                amount.hash(state);
            }
        }

        state.update(b"history\0");
        for (height, transactions) in &self.history {
            state.update(b"height\0");
            state.update(height.to_be_bytes());
            state.update(b"transactions\0");
            for t in transactions {
                t.hash(state);
                state.update(b"\0");
            }
        }
    }
}
