use crate::application::error::{Error, Result};
use omni::Identity;
use sha3::{Digest, Sha3_256};
use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, Sender};
use tracing::debug;

/// Manages key/value store state.
#[derive(Debug)]
pub struct KeyValueStoreDriver {
    balances: BTreeMap<Identity, u128>,
    height: u64,
    cmd_rx: Receiver<Command>,
}

impl KeyValueStoreDriver {
    pub fn new(cmd_rx: Receiver<Command>) -> Self {
        Self {
            balances: Default::default(),
            height: 0,
            cmd_rx,
        }
    }

    /// Run the driver in the current thread (blocking).
    pub fn run(mut self) -> Result<()> {
        fn send<V>(tx: Sender<V>, v: V) -> Result<()> {
            tx.send(v).map_err(|e| Error::ChannelSend(e.to_string()))
        }

        'main: loop {
            match self.cmd_rx.recv()? {
                Command::Info { result_tx } => {
                    send(result_tx, (self.height, self.hash()))?;
                }
                Command::Commit { result_tx } => {
                    send(result_tx, (self.height, self.hash()))?;
                }
                Command::Mint {
                    account,
                    amount,
                    result_tx,
                } => {
                    debug!(
                        "Minting {} for account 0x{}",
                        amount,
                        hex::encode(&account.to_vec())
                    );

                    *self.balances.entry(account).or_default() += amount;
                    send(result_tx, ())?;
                }
                Command::QueryBalance { account, result_tx } => {
                    let balance = self.balances.get(&account).map_or(0, |e| *e);
                    send(result_tx, (balance, self.height))?;
                }

                Command::SendTokens {
                    from,
                    to,
                    amount,
                    result_tx,
                } => {
                    let amount_from = self.balances.get(&from).map_or(0, |e| *e);
                    let amount_to = self.balances.get(&to).map_or(0, |e| *e);

                    if amount > amount_from {
                        send(result_tx, Err("Not enough funds".to_string()))?;
                        continue 'main;
                    }

                    match amount_from.checked_sub(amount) {
                        None => send(result_tx, Err("Not enough funds".to_string()))?,
                        Some(new_from) => match amount_to.checked_add(amount) {
                            None => send(result_tx, Err("Would overflow.".to_string()))?,
                            Some(new_to) => {
                                self.balances.insert(from, new_from);
                                self.balances.insert(to, new_to);
                                send(result_tx, Ok(()));
                            }
                        },
                    }
                }
            }
        }
    }

    fn hash(&self) -> Vec<u8> {
        let mut hasher = Sha3_256::default();
        for (k, v) in self.balances.iter() {
            hasher.update(b"\x0Alabel\0");
            hasher.update(&k.to_vec());
            hasher.update(b"\x0Avalue\0");
            hasher.update(v.to_be_bytes());
        }

        hasher.finalize().to_vec()
    }

    fn commit(&mut self, result_tx: Sender<(u64, Vec<u8>)>) -> Result<()> {
        self.height += 1;

        Ok(result_tx
            .send((self.height, self.hash()))
            .map_err(|e| Error::ChannelSend(e.to_string()))?)
    }
}

#[derive(Debug, Clone)]
pub enum Command {
    Info {
        result_tx: Sender<(u64, Vec<u8>)>,
    },
    Commit {
        result_tx: Sender<(u64, Vec<u8>)>,
    },

    SendTokens {
        from: Identity,
        to: Identity,
        amount: u128,
        result_tx: Sender<std::result::Result<(), String>>,
    },

    QueryBalance {
        account: Identity,
        result_tx: Sender<(u128, u64)>,
    },

    Mint {
        account: Identity,
        amount: u128,
        result_tx: Sender<()>,
    },
}
