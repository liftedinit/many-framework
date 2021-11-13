use async_trait::async_trait;
use clap::Parser;
use minicbor::data::Tag;
use minicbor::encode::{Error, Write};
use minicbor::{decode, Decode, Decoder, Encode, Encoder};
use omni::message::error::define_omni_error;
use omni::message::{RequestMessage, ResponseMessage};
use omni::protocol::Attribute;
use omni::server::module::{OmniModule, OmniModuleInfo};
use omni::server::OmniServer;
use omni::transport::http::HttpServer;
use omni::{Identity, OmniError};
use omni_abci::module::{AbciInfo, AbciInit, OmniAbciModuleBackend};
use sha3::Digest;
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Formatter;
use std::iter::FromIterator;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

define_omni_error!(
    attribute 2 => {
        1: fn unknown_symbol(symbol) => "Symbol not supported by this ledger: {symbol}.",
        2: fn unauthorized() => "Unauthorized to do this operation.",
        3: fn insufficient_funds() => "Insufficient funds.",
        4: fn anonymous_cannot_hold_funds() => "Anonymous is not a valid account identity.",
    }
);

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
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
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

const LEDGER_ATTRIBUTE: Attribute = Attribute::new(
    2,
    &[
        "ledger.info",
        "ledger.balance",
        "ledger.mint",
        "ledger.burn",
        "ledger.send",
    ],
);

lazy_static::lazy_static!(
    pub static ref LEDGER_MODULE_INFO: OmniModuleInfo = OmniModuleInfo {
        name: "LedgerModule".to_string(),
        attributes: vec![LEDGER_ATTRIBUTE],
    };
);

#[derive(Default)]
struct LedgerStorage {
    pub accounts: BTreeMap<Identity, BTreeMap<String, TokenAmount>>,
    pub history: BTreeMap<u64, Vec<Transaction>>,
    pub height: u64,

    hash_cache: Cell<Option<Vec<u8>>>,
}

impl std::fmt::Debug for LedgerStorage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
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
    }

    pub fn add_transaction(&mut self, tx: Transaction) {
        self.hash_cache.take();
        self.history.entry(self.height).or_default().push(tx);
    }

    pub fn get_balance(&self, identity: &Identity, symbol: &str) -> Option<&TokenAmount> {
        if identity.is_anonymous() {
            None
        } else {
            self.accounts.get(identity)?.get(symbol).map(|x| x)
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
            return Err(anonymous_cannot_hold_funds());
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
            return Err(anonymous_cannot_hold_funds());
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
            return Err(anonymous_cannot_hold_funds());
        }

        let amount_from = self.get_balance(from, symbol).cloned().unwrap_or_default();
        if amount > amount_from {
            return Err(insufficient_funds());
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

    fn hash(&self) -> Vec<u8> {
        let cache = self.hash_cache.as_ptr();

        if let Some(cache) = unsafe { &*cache } {
            cache.clone()
        } else {
            let mut hasher = sha3::Sha3_512::new();
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

/// A simple ledger that keeps transactions in memory.
#[derive(Default, Debug, Clone)]
pub struct LedgerModule {
    owner_id: Identity,
    symbols: BTreeSet<String>,
    default_symbol: String,
    storage: Arc<Mutex<LedgerStorage>>,
}

impl LedgerModule {
    pub fn new(owner_id: Identity, symbols: Vec<String>, default_symbol: String) -> Self {
        Self {
            owner_id,
            symbols: BTreeSet::from_iter(symbols.iter().map(|x| x.to_string())),
            default_symbol,
            ..Default::default()
        }
    }

    /// Checks whether an identity is the owner or not. Anonymous identities are forbidden.
    pub fn is_owner(&self, identity: &Identity) -> bool {
        &self.owner_id == identity && !identity.is_anonymous()
    }

    pub fn is_known_symbol(&self, symbol: &str) -> Result<(), OmniError> {
        if self.symbols.contains(symbol) {
            Ok(())
        } else {
            Err(unknown_symbol(symbol.to_string()))
        }
    }

    fn info(&self, _payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let mut bytes = Vec::with_capacity(512);
        let mut e = Encoder::new(&mut bytes);
        let storage = self.storage.lock().unwrap();

        // Hash the storage.
        let hash = storage.hash();

        e.begin_map()
            .and_then(move |e| {
                e.str("height")?.u64(storage.height)?;
                e.str("hash")?.bytes(hash.as_slice())?;
                e.str("symbols")?.encode(&self.symbols)?;
                e.str("default_symbol")?.str(self.default_symbol.as_str())?;

                e.end()?;
                Ok(())
            })
            .map_err(|e| OmniError::serialization_error(e.to_string()))?;

        Ok(bytes)
    }

    fn balance(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let mut d = minicbor::Decoder::new(payload);
        let (identity, symbol): (Option<Identity>, Option<&str>) = d
            .decode()
            .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
        let symbol = symbol.unwrap_or_else(|| self.default_symbol.as_str());

        let storage = self.storage.lock().unwrap();
        if let Some(amount) = storage.get_balance(identity.as_ref().unwrap_or_else(|| from), symbol)
        {
            minicbor::to_vec(amount).map_err(|e| OmniError::serialization_error(e.to_string()))
        } else {
            minicbor::to_vec(TokenAmount::zero())
                .map_err(|e| OmniError::serialization_error(e.to_string()))
        }
    }

    fn send(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let mut d = minicbor::Decoder::new(payload);
        let (to, amount, symbol): (Identity, TokenAmount, Option<&str>) = d
            .decode()
            .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
        let symbol = symbol.unwrap_or_else(|| self.default_symbol.as_str());

        let mut storage = self.storage.lock().unwrap();
        storage.send(&from, &to, symbol, amount.clone())?;

        if let Some(amount) = storage.get_balance(&from, symbol) {
            minicbor::to_vec(amount).map_err(|e| OmniError::serialization_error(e.to_string()))
        } else {
            minicbor::to_vec(TokenAmount::zero())
                .map_err(|e| OmniError::serialization_error(e.to_string()))
        }
    }

    fn mint(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        if !self.is_owner(from) {
            return Err(unauthorized());
        }

        let mut d = minicbor::Decoder::new(payload);
        let (to, amount, symbol): (Identity, TokenAmount, Option<&str>) = d
            .decode()
            .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
        let symbol = symbol.unwrap_or_else(|| self.default_symbol.as_str());

        let mut storage = self.storage.lock().unwrap();
        storage.mint(&to, symbol, amount)?;

        if let Some(amount) = storage.get_balance(&to, symbol) {
            minicbor::to_vec(amount).map_err(|e| OmniError::serialization_error(e.to_string()))
        } else {
            minicbor::to_vec(TokenAmount::zero())
                .map_err(|e| OmniError::serialization_error(e.to_string()))
        }
    }

    fn burn(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        if !self.is_owner(from) {
            return Err(unauthorized());
        }

        let mut d = minicbor::Decoder::new(payload);
        let (to, amount, symbol): (Identity, TokenAmount, Option<String>) = d
            .decode()
            .map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let mut storage = self.storage.lock().unwrap();
        storage.burn(
            &to,
            symbol.as_ref().unwrap_or_else(|| &self.default_symbol),
            amount,
        )?;

        Ok(Vec::new())
    }
}

impl OmniAbciModuleBackend for LedgerModule {
    fn init(&self) -> AbciInit {
        AbciInit {
            endpoints: BTreeMap::from([
                ("ledger.info".to_string(), false),
                ("ledger.balance".to_string(), false),
                ("ledger.mint".to_string(), true),
                ("ledger.burn".to_string(), true),
                ("ledger.send".to_string(), true),
            ]),
        }
    }

    fn init_chain(&self) -> Result<(), OmniError> {
        Ok(())
    }

    fn info(&self) -> Result<AbciInfo, OmniError> {
        let storage = self.storage.lock().unwrap();
        Ok(AbciInfo {
            height: storage.height,
            hash: storage.hash(),
        })
    }

    fn commit(&self) -> Result<(), OmniError> {
        let mut storage = self.storage.lock().unwrap();
        Ok(storage.commit())
    }
}

#[async_trait]
impl OmniModule for LedgerModule {
    fn info(&self) -> &OmniModuleInfo {
        &LEDGER_MODULE_INFO
    }

    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        let symbol = match message.method.as_str() {
            "ledger.info" => return Ok(()),
            "ledger.mint" => {
                decode::<'_, (Identity, TokenAmount, Option<&str>)>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?
                    .2
            }
            "ledger.burn" => {
                decode::<'_, (Identity, TokenAmount, Option<&str>)>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?
                    .2
            }
            "ledger.balance" => {
                decode::<'_, (Option<Identity>, Option<&str>)>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?
                    .1
            }
            "ledger.send" => {
                decode::<'_, (Identity, TokenAmount, Option<&str>)>(message.data.as_slice())
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))?
                    .2
            }
            _ => {
                return Err(OmniError::internal_server_error());
            }
        };
        symbol.map_or(Ok(()), |s| self.is_known_symbol(s))
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let data = match message.method.as_str() {
            "ledger.info" => self.info(&message.data),
            "ledger.balance" => self.balance(&message.from.unwrap_or_default(), &message.data),
            "ledger.mint" => self.mint(&message.from.unwrap_or_default(), &message.data),
            "ledger.burn" => self.burn(&message.from.unwrap_or_default(), &message.data),
            "ledger.send" => self.send(&message.from.unwrap_or_default(), &message.data),
            _ => Err(OmniError::internal_server_error()),
        }?;

        Ok(ResponseMessage::from_request(
            &message,
            &message.to,
            Ok(data),
        ))
    }
}

#[derive(Parser)]
struct Opts {
    /// The location of a Ed25519 PEM file for the identity of this server.
    #[clap(long)]
    pem: PathBuf,

    /// The pem file for the owner of the ledger (who can mint).
    #[clap(long)]
    owner: PathBuf,

    /// The port to bind to for the OMNI Http server.
    #[clap(long, short, default_value = "8000")]
    port: u16,

    /// The list of supported symbols.
    #[clap(long, short)]
    symbols: Vec<String>,

    /// The default symbol to use. If unspecified, will use the first symbol in the list of symbols.
    #[clap(long, short)]
    default: Option<String>,

    /// Uses an ABCI application module.
    #[clap(long)]
    abci: bool,
}

fn main() {
    let Opts {
        pem,
        owner,
        port,
        symbols,
        default,
        abci,
    } = Opts::parse();
    let default = default.unwrap_or(symbols.first().unwrap().to_string());
    let (id, keypair) = Identity::from_pem_addressable(std::fs::read(pem).unwrap()).unwrap();
    let (owner_id, _) = Identity::from_pem_public(std::fs::read(owner).unwrap()).unwrap();

    let module = LedgerModule::new(owner_id, symbols, default);
    let omni = OmniServer::new("omni-ledger", id, &keypair);
    let omni = if abci {
        omni.with_module(omni_abci::module::AbciModule::new(module))
    } else {
        omni.with_module(module)
    };

    HttpServer::simple(id, Some(keypair), omni)
        .bind(format!("127.0.0.1:{}", port))
        .unwrap();
}
