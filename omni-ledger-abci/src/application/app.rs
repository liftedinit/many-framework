//! In-memory OMNI Ledger Registry as an ABCI application.
use crate::application::{Command, KeyValueStoreDriver};
use omni::cbor::cose::CoseSign1;
use omni::cbor::message::RequestMessage;
use omni::cbor::value::CborValue;
use omni::Identity;
use std::convert::TryFrom;
use std::sync::mpsc::{channel, Sender};
use tendermint_abci::Application;
use tendermint_proto::abci::{
    Event, EventAttribute, RequestCheckTx, RequestDeliverTx, RequestInfo, RequestQuery,
    ResponseCheckTx, ResponseCommit, ResponseDeliverTx, ResponseInfo, ResponseQuery,
};
use tracing::{debug, info};

fn from_der(der: &[u8]) -> Result<Vec<u8>, String> {
    use simple_asn1::{
        from_der, oid,
        ASN1Block::{BitString, ObjectIdentifier, Sequence},
    };

    let object = from_der(der).map_err(|e| format!("asn error: {:?}", e))?;
    let first = object.first().ok_or(format!("empty object"))?;

    match first {
        Sequence(_, blocks) => {
            let algorithm = blocks.get(0).ok_or(format!("Invalid ASN1"))?;
            let bytes = blocks.get(1).ok_or(format!("Invalid ASN1"))?;
            let id_ed25519 = oid!(1, 3, 101, 112);
            match (algorithm, bytes) {
                (Sequence(_, oid_sequence), BitString(_, _, bytes)) => match oid_sequence.first() {
                    Some(ObjectIdentifier(_, oid)) => {
                        if oid == id_ed25519 {
                            Ok(bytes.clone())
                        } else {
                            Err(format!("Invalid oid."))
                        }
                    }
                    _ => Err(format!("Invalid oid.")),
                },
                _ => Err(format!("Invalid oid.")),
            }
        }
        _ => Err(format!("Invalid root type."))?,
    }
}

/// In-memory, hashmap-backed key/value store ABCI application.
///
/// This structure effectively just serves as a handle to the actual key/value
/// store - the [`KeyValueStoreDriver`].
#[derive(Debug, Clone)]
pub struct KeyValueStoreApp {
    cmd_tx: Sender<Command>,
}

impl KeyValueStoreApp {
    /// Constructor.
    pub fn new() -> (Self, KeyValueStoreDriver) {
        let (cmd_tx, cmd_rx) = channel();
        (Self { cmd_tx }, KeyValueStoreDriver::new(cmd_rx))
    }

    fn get_key_for_identity(
        &self,
        cose_sign1: &CoseSign1,
        kid: Vec<u8>,
    ) -> Option<ring::signature::UnparsedPublicKey<Vec<u8>>> {
        let v = cose_sign1
            .protected
            .custom_headers
            .get(&CborValue::TextString("keys".to_string()))?;

        let key_bytes = match v {
            CborValue::Map(ref m) => {
                let value = m.get(&CborValue::ByteString(kid.clone()))?;
                match value {
                    CborValue::ByteString(value) => Some(value),
                    _ => None,
                }
            }
            _ => None,
        }?;

        // Verify the keybytes matches the identity.
        let id = Identity::try_from(kid.as_slice()).ok()?;
        if id.is_anonymous() {
            return None;
        } else if id.is_public_key() {
            let other = Identity::public_key(key_bytes.to_vec());
            if other == id {
                Some(ring::signature::UnparsedPublicKey::new(
                    &ring::signature::ED25519,
                    from_der(key_bytes).ok()?,
                ))
            } else {
                None
            }
        } else if id.is_addressable() {
            if Identity::addressable(key_bytes.to_vec()) == id {
                Some(ring::signature::UnparsedPublicKey::new(
                    &ring::signature::ED25519,
                    key_bytes.to_owned(),
                ))
            } else {
                None
            }
        } else {
            None
        }
    }

    // TODO: add verification of the `to` fields.
    fn verify(&self, cose_sign1: &CoseSign1) -> bool {
        if let Some(ref kid) = cose_sign1.protected.key_identifier {
            if let Ok(id) = Identity::from_bytes(kid) {
                if id.is_anonymous() {
                    // TODO: allow anonymous requests IF THEY MATCH the message's from field.
                    return false;
                }
            }

            self.get_key_for_identity(cose_sign1, kid.clone())
                .map(|key| {
                    cose_sign1
                        .verify_with(|content, sig| key.verify(content, sig).is_ok())
                        .unwrap_or(false)
                })
                .unwrap_or(false)
        } else {
            false
        }
    }

    fn decode_and_verify(&self, bytes: &[u8]) -> Result<RequestMessage, String> {
        let cose_sign1 = minicbor::decode::<CoseSign1>(bytes)
            .map_err(|e| format!("Invalid COSE CBOR message: {}", e))?;

        if !self.verify(&cose_sign1) {
            return Err("Could not verify the signature.".to_string());
        }

        if let Some(payload) = cose_sign1.payload {
            let mut message = RequestMessage::from_bytes(&payload)?;

            // Update `from` and `to` if they're missing.
            message.from = match message.from {
                None => Some(
                    Identity::from_bytes(&cose_sign1.protected.key_identifier.unwrap_or_default())
                        .map_err(|e| format!("{:?}", e))?,
                ),
                Some(from) => Some(from),
            };

            // TODO: add `to` overload with the threshold key from this blockchain.

            Ok(message)
        } else {
            Err("payload missing".to_string())
        }
    }
}

impl Application for KeyValueStoreApp {
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
        let message = match self.decode_and_verify(&request.data) {
            Ok(message) => message,
            Err(e) => {
                return ResponseQuery {
                    code: 1,
                    log: e,
                    ..Default::default()
                }
            }
        };
        let from = match message.from {
            Some(f) => f,
            None => {
                return ResponseQuery {
                    code: 2,
                    ..Default::default()
                }
            }
        };

        match message.method.as_str() {
            "balance" => {
                let (result_tx, result_rx) = channel();
                let account = message.from.unwrap();

                self.cmd_tx
                    .send(Command::QueryBalance {
                        account: account.clone(),
                        result_tx,
                    })
                    .unwrap();
                let (amount, height) = result_rx.recv().unwrap();
                ResponseQuery {
                    code: 0,
                    key: account.to_vec(),
                    value: amount.to_be_bytes().to_vec(),
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
        let (code, log) = match self.decode_and_verify(&request.tx) {
            Ok(_) => (0, "".to_string()),
            Err(e) => (1, e),
        };

        ResponseCheckTx {
            code,
            data: vec![],
            log,
            info: "".to_string(),
            gas_wanted: 1,
            gas_used: 0,
            events: vec![],
            codespace: "".to_string(),
        }
    }

    fn deliver_tx(&self, request: RequestDeliverTx) -> ResponseDeliverTx {
        let message = match self.decode_and_verify(&request.tx) {
            Ok(message) => message,
            Err(e) => {
                return ResponseDeliverTx {
                    code: 1,
                    log: e,
                    ..Default::default()
                };
            }
        };

        match message.method.as_str() {
            "mint" => {
                // TODO: limit this to an owner public key that's passed during initialization.
                if message.from().to_string()
                    != "ozhagjlfre6vtu7aucntdddtijmfp2x4cmplhmodn2y6b3upyi"
                {
                    return ResponseDeliverTx {
                        code: 3,
                        log: "unauthorized".to_string(),
                        ..Default::default()
                    };
                }

                let account = message.from.unwrap();
                let data = message.data.unwrap_or_default();
                let mut d = minicbor::Decoder::new(&data);
                let args = d
                    .array()
                    .and_then(|_| {
                        d.decode::<Identity>().and_then(|id| {
                            d.decode::<u64>().and_then(|amount_big| {
                                d.decode::<u64>().and_then(|amount_little| {
                                    Ok((id, (amount_big as u128) << 64 + (amount_little as u128)))
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
            "send" => {
                let from = message.from();
                if from.is_anonymous() {
                    return ResponseDeliverTx {
                        code: 5,
                        ..Default::default()
                    };
                }

                let data = message.data.unwrap_or_default();
                let mut d = minicbor::Decoder::new(&data);
                let args = d
                    .array()
                    .and_then(|_| {
                        d.decode::<Identity>().and_then(|id| {
                            d.decode::<u64>().and_then(|amount_big| {
                                d.decode::<u64>().and_then(|amount_little| {
                                    Ok((id, (amount_big as u128) << 64 + (amount_little as u128)))
                                })
                            })
                        })
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

                result_rx.recv().unwrap();
                ResponseDeliverTx {
                    code: 0,
                    ..Default::default()
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
