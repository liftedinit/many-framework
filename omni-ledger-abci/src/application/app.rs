use crate::application::{Command, LedgerApplicationDriver};
use minicose::CoseSign1;
use omni::Identity;
use std::ops::Shl;
use std::sync::mpsc::{channel, Sender};
use tendermint_abci::Application;
use tendermint_proto::abci::{
    RequestCheckTx, RequestDeliverTx, RequestInfo, RequestQuery, ResponseCheckTx, ResponseCommit,
    ResponseDeliverTx, ResponseInfo, ResponseQuery,
};
use tracing::{debug, info};

/// In-memory, hashmap-backed key/value store ABCI application.
///
/// This structure effectively just serves as a handle to the actual key/value
/// store - the [`KeyValueStoreDriver`].
#[derive(Debug, Clone)]
pub struct LedgerAbciApp {
    cmd_tx: Sender<Command>,
}

impl LedgerAbciApp {
    /// Constructor.
    pub fn new() -> (Self, LedgerApplicationDriver) {
        let (cmd_tx, cmd_rx) = channel();
        (Self { cmd_tx }, LedgerApplicationDriver::new(cmd_rx))
    }
}

impl Application for LedgerAbciApp {
    fn info(&self, request: RequestInfo) -> ResponseInfo {
        debug!(
            "Got info request. Tendermint version: {}; Block version: {}; P2P version: {}",
            request.version, request.block_version, request.p2p_version
        );

        let (result_tx, result_rx) = channel();
        self.cmd_tx.send(Command::Info { result_tx }).unwrap();
        let (last_block_height, last_block_app_hash) = result_rx.recv().unwrap();

        ResponseInfo {
            data: "omni-ledger".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            app_version: 1,
            last_block_height: last_block_height as i64,
            last_block_app_hash: last_block_app_hash.to_vec(),
        }
    }

    fn query(&self, request: RequestQuery) -> ResponseQuery {
        let cose_sign = match CoseSign1::from_bytes(&request.data) {
            Ok(cose) => cose,
            Err(e) => {
                return ResponseQuery {
                    code: 1,
                    log: e.to_string(),
                    ..Default::default()
                }
            }
        };
        let message = match omni::message::decode_request_from_cose_sign1(cose_sign) {
            Ok(message) => message,
            Err(e) => {
                return ResponseQuery {
                    code: 1,
                    log: e.to_string(),
                    ..Default::default()
                }
            }
        };
        if message.from.is_none() {
            return ResponseQuery {
                code: 2,
                ..Default::default()
            };
        }

        match message.method.as_str() {
            "ledger.balance" => {
                let (result_tx, result_rx) = channel();
                let account = message.from.unwrap();

                self.cmd_tx
                    .send(Command::QueryBalance {
                        account: account.clone(),
                        result_tx,
                    })
                    .unwrap();
                let (amount, height) = result_rx.recv().unwrap();
                let mut bytes = Vec::<u8>::new();
                let mut e = minicbor::Encoder::new(&mut bytes);
                e.array(2)
                    .unwrap()
                    .u64((amount >> 64) as u64)
                    .unwrap()
                    .u64(amount as u64)
                    .unwrap();
                eprintln!("data: {}", hex::encode(&bytes));

                ResponseQuery {
                    code: 0,
                    key: account.to_vec(),
                    value: bytes,
                    height: height as i64,
                    ..Default::default()
                }
            }
            _ => ResponseQuery {
                code: 2,
                ..Default::default()
            },
        }
    }

    fn check_tx(&self, request: RequestCheckTx) -> ResponseCheckTx {
        let cose_sign = match CoseSign1::from_bytes(&request.tx) {
            Ok(cose) => cose,
            Err(e) => {
                return ResponseCheckTx {
                    code: 1,
                    log: e.to_string(),
                    ..Default::default()
                }
            }
        };

        let (code, log) = match omni::message::decode_request_from_cose_sign1(cose_sign) {
            Ok(_) => (0, "".to_string()),
            Err(e) => (1, e.to_string()),
        };

        ResponseCheckTx {
            code,
            data: vec![],
            log,
            ..Default::default()
        }
    }

    fn deliver_tx(&self, request: RequestDeliverTx) -> ResponseDeliverTx {
        let cose_sign = match CoseSign1::from_bytes(&request.tx) {
            Ok(cose) => cose,
            Err(e) => {
                return ResponseDeliverTx {
                    code: 1,
                    log: e.to_string(),
                    ..Default::default()
                }
            }
        };

        let message = match omni::message::decode_request_from_cose_sign1(cose_sign) {
            Ok(message) => message,
            Err(e) => {
                return ResponseDeliverTx {
                    code: 1,
                    log: e.to_string(),
                    ..Default::default()
                };
            }
        };

        match message.method.as_str() {
            "ledger.mint" => {
                // TODO: limit this to an owner public key that's passed during initialization.
                if message.from().to_string()
                    != "oysahek5lid7ek7ckhq7j77nfwgk3vkspnyppm2u467ne5mwiq"
                {
                    return ResponseDeliverTx {
                        code: 3,
                        log: "unauthorized".to_string(),
                        ..Default::default()
                    };
                }

                let data = message.data;
                let mut d = minicbor::Decoder::new(&data);
                let args = d
                    .array()
                    .and_then(|_| {
                        d.decode::<Identity>().and_then(|id| {
                            d.decode::<u64>().and_then(|amount_big| {
                                d.decode::<u64>().and_then(|amount_little| {
                                    Ok((id, (amount_big as u128).shl(64) + (amount_little as u128)))
                                })
                            })
                        })
                    })
                    .map_err(|e| ResponseDeliverTx {
                        code: 5,
                        log: format!("invalid data: {:?}", e),
                        ..Default::default()
                    });

                let (account, amount) = match args {
                    Ok(x) => x,
                    Err(e) => {
                        return e;
                    }
                };

                let (result_tx, result_rx) = channel();
                self.cmd_tx
                    .send(Command::Mint {
                        account,
                        amount,
                        result_tx,
                    })
                    .unwrap();
                result_rx.recv().unwrap();
                ResponseDeliverTx {
                    code: 0,
                    ..Default::default()
                }
            }
            "ledger.send" => {
                let from = message.from();
                if from.is_anonymous() {
                    return ResponseDeliverTx {
                        code: 5,
                        ..Default::default()
                    };
                }

                let data = message.data;
                let mut d = minicbor::Decoder::new(&data);
                let args = d
                    .array()
                    .and_then(|_| {
                        Ok((d.decode::<Identity>()?, (d.u64()? as u128) << 64 + d.u64()?))
                    })
                    .map_err(|e| ResponseDeliverTx {
                        code: 5,
                        log: format!("invalid data: {:?}", e),
                        ..Default::default()
                    });

                let (to, amount) = match args {
                    Ok(x) => x,
                    Err(e) => {
                        return e;
                    }
                };

                let (result_tx, result_rx) = channel();
                self.cmd_tx
                    .send(Command::SendTokens {
                        from,
                        to,
                        amount,
                        result_tx,
                    })
                    .unwrap();

                match result_rx.recv().unwrap() {
                    Ok(()) => ResponseDeliverTx {
                        code: 0,
                        ..Default::default()
                    },
                    Err(msg) => ResponseDeliverTx {
                        code: 6,
                        log: msg,
                        ..Default::default()
                    },
                }
            }
            _ => ResponseDeliverTx {
                code: 2,
                log: "not found".to_string(),
                ..Default::default()
            },
        }
    }

    fn commit(&self) -> ResponseCommit {
        let (result_tx, result_rx) = channel();
        self.cmd_tx.send(Command::Commit { result_tx }).unwrap();
        let (height, app_hash) = result_rx.recv().unwrap();
        info!(
            "Committed height {}, hash {}",
            height,
            hex::encode(&app_hash)
        );
        ResponseCommit {
            data: app_hash.to_vec(),
            retain_height: 0,
        }
    }
}
