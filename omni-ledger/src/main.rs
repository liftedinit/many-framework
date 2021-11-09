use async_trait::async_trait;
use clap::Parser;
use minicbor::data::Type;
use omni::message::{RequestMessage, ResponseMessage};
use omni::protocol::Attribute;
use omni::server::module::{OmniModule, OmniModuleInfo};
use omni::server::OmniServer;
use omni::transport::http::HttpServer;
use omni::{Identity, OmniError};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use omni::message::error::define_omni_error;

define_omni_error!(
    2 => {
        1: fn unauthorized() => "Unauthorized to do this operation.",
        2: fn insufficient_funds() => "Insufficient funds.",
        3: fn would_overflow() => "Doing this operation would overflow the account.",
    }
);

#[derive(Parser)]
struct Opts {
    /// The location of a Ed25519 PEM file for the identity of this server.
    #[clap(long)]
    pem: PathBuf,

    /// The pem file for the owner of the ledger (who can mint).
    #[clap(long)]
    owner: PathBuf,

    /// The port to bind to for the OMNI Http server.
    #[clap(long, default_value = "8000")]
    port: u16,
}

#[derive(Debug, Clone)]
pub enum Transaction {
    Mint(Identity, u128),
    Send(Identity, Identity, u128),
}

const LEDGER_ATTRIBUTE: Attribute = Attribute {
    id: 2,
    endpoints: &["ledger.balance", "ledger.mint", "ledger.send"],
};
lazy_static::lazy_static!(

    pub static ref LEDGER_MODULE_INFO: OmniModuleInfo = OmniModuleInfo {
        name: "LedgerModule".to_string(),
        attributes: vec![LEDGER_ATTRIBUTE],
    };
);

#[derive(Default, Debug, Clone)]
struct LedgerStorage {
    pub accounts: BTreeMap<Identity, BTreeMap<String, u128>>,
    pub history: Vec<(u64, Vec<Transaction>)>,
    pub height: u64,
}

impl LedgerStorage {
    pub fn commit(&mut self) -> () {
        self.height += 1;
    }

    pub fn get_balance(&self, identity: &Identity, ticker: &str) -> Option<u128> {
        self.accounts.get(identity)?.get(ticker).map(|x| *x)
    }

    pub fn get_transactions_at(&self, index: u64) -> &[Transaction] {
        if let Ok(i) = self.history.binary_search_by_key(&index, |(i, _)| *i) {
            self.history[i].1.as_slice()
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
                    x @ Transaction::Mint(d, _) if d == account => {
                        result.push(x);
                    }
                    x @ Transaction::Send(f, _, _) if f == account => {
                        result.push(x);
                    }
                    x @ Transaction::Send(_, t, _) if t == account => {
                        result.push(x);
                    }
                    _ => {}
                }
            }
        }

        result
    }

    pub fn mint(&mut self, to: &Identity, ticker: &str, amount: u128) -> Result<(), OmniError> {
        *self
            .accounts
            .entry(to.clone())
            .or_default()
            .entry(ticker.to_string())
            .or_default() += amount;

        Ok(())
    }

    pub fn send(
        &mut self,
        from: &Identity,
        to: &Identity,
        ticker: &str,
        amount: u128,
    ) -> Result<(), OmniError> {
        if amount == 0 {
            // NOOP.
            return Ok(());
        }

        let amount_from = self.get_balance(from, ticker).unwrap_or(0);
        let amount_to = self.get_balance(to, ticker).unwrap_or(0);

        if amount > amount_from {
            return Err(insufficient_funds());
        }

        match amount_from.checked_sub(amount) {
            None => {
                return Err(insufficient_funds());
            }
            Some(new_from) => match amount_to.checked_add(amount) {
                None => {
                    return Err(would_overflow());
                }
                Some(new_to) => {
                    *self
                        .accounts
                        .entry(*from)
                        .or_default()
                        .entry(ticker.to_string())
                        .or_default() = new_from;
                    *self
                        .accounts
                        .entry(*to)
                        .or_default()
                        .entry(ticker.to_string())
                        .or_default() = new_to;
                }
            },
        }
        Ok(())
    }
}

impl Hash for LedgerStorage {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(b"accounts\0");
        for (k, accounts) in &self.accounts {
            state.write(b"a\0");
            state.write(&k.to_vec());
            state.write(b"t\0");

            for (ticker, amount) in accounts {
                state.write(ticker.as_bytes());
                state.write(b"\0");
                state.write_u128(*amount);
                state.write(b"\0");
            }
        }
    }
}

/// A simple ledger that keeps transactions in memory.
#[derive(Default, Debug, Clone)]
pub struct LedgerModule {
    owner_id: Identity,
    storage: Arc<Mutex<LedgerStorage>>,
}

impl LedgerModule {
    pub fn new(owner_id: Identity) -> Self {
        Self {
            owner_id,
            ..Default::default()
        }
    }

    /// Checks whether an identity is the owner or not. Anonymous identities are forbidden.
    pub fn is_owner(&self, identity: &Identity) -> bool {
        &self.owner_id == identity && !identity.is_anonymous()
    }

    fn balance(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let mut d = minicbor::Decoder::new(payload);
        let (identity, ticker) = d
            .array()
            .and_then(|_| {
                let identity = d.decode::<Identity>().unwrap_or_else(|_| from.clone());
                let ticker = d.str().unwrap_or("FBT");
                Ok((identity, ticker))
            })
            .map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let mut storage = self.storage.lock().unwrap();
        let amount = storage.get_balance(&identity, ticker).unwrap_or(0);
        let mut bytes = Vec::<u8>::new();
        minicbor::encode(((amount >> 64) as u64, amount as u64), &mut bytes)
            .map_err(|e| OmniError::serialization_error(e.to_string()))?;

        Ok(bytes)
    }

    fn send(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        let mut d = minicbor::Decoder::new(payload);
        let (to, amount, ticker) = d
            .array()
            .and_then(|_| {
                let i = d.decode::<Identity>()?;
                let hi = d.u64()?;
                let low = d.u64()?;
                let t = match d.datatype() {
                    Ok(Type::String) => d.str()?,
                    _ => "FBT",
                };

                Ok((i, ((hi as u128) << 64) + (low as u128), t))
            })
            .map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let mut storage = self.storage.lock().unwrap();
        storage.send(&from, &to, &ticker, amount)?;

        Ok(Vec::new())
    }

    fn mint(&self, from: &Identity, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        if !self.is_owner(from) {
            return Err(unauthorized());
        }

        let mut d = minicbor::Decoder::new(payload);
        let (to, amount, ticker) = d
            .array()
            .and_then(|_| {
                let i = d.decode::<Identity>()?;
                let hi = d.u64()?;
                let low = d.u64()?;
                let t = match d.datatype() {
                    Ok(Type::String) => d.str()?,
                    _ => "FBT",
                };

                Ok((i, ((hi as u128) << 64) + (low as u128), t))
            })
            .map_err(|e| OmniError::deserialization_error(e.to_string()))?;

        let mut storage = self.storage.lock().unwrap();
        storage.mint(&to, &ticker, amount)?;

        Ok(Vec::new())
    }
}

#[async_trait]
impl OmniModule for LedgerModule {
    fn info(&self) -> &OmniModuleInfo {
        &LEDGER_MODULE_INFO
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let data = match message.method.as_str() {
            "ledger.mint" => self.mint(&message.from.unwrap_or_default(), &message.data),
            "ledger.balance" => self.balance(&message.from.unwrap_or_default(), &message.data),
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

fn main() {
    let o: Opts = Opts::parse();
    let (id, keypair) = Identity::from_pem_addressable(std::fs::read(o.pem).unwrap()).unwrap();
    let (owner_id, _) = Identity::from_pem_public(std::fs::read(o.owner).unwrap()).unwrap();

    let omni = OmniServer::new(id, &keypair).with_module(LedgerModule::new(owner_id));

    HttpServer::simple(id, Some(keypair), omni)
        .bind(format!("127.0.0.1:{}", o.port))
        .unwrap();
}
