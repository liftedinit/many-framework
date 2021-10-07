//! In-memory OMNI Ledger Registry as an ABCI application.
use crate::application::{Command, KeyValueStoreDriver};
use std::sync::mpsc::{channel, Sender};
use tendermint_abci::Application;
use tendermint_proto::abci::{
    Event, EventAttribute, RequestCheckTx, RequestDeliverTx, RequestInfo, RequestQuery,
    ResponseCheckTx, ResponseCommit, ResponseDeliverTx, ResponseInfo, ResponseQuery,
};
use tracing::{debug, info};

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
        // let key = match String::from_utf8(request.data.clone()) {
        //     Ok(s) => s,
        //     Err(e) => panic!("Failed to intepret key as UTF-8: {}", e),
        // };
        // debug!("Attempting to get key: {}", key);
        // match self.get(key.clone()) {
        //     Ok((height, value_opt)) => match value_opt {
        //         Some(value) =>
        ResponseQuery {
            code: 0,
            log: "exists".to_string(),
            info: "".to_string(),
            index: 0,
            key: request.data,
            value: vec![], // value.into_bytes(),
            proof_ops: None,
            height: 0,
            codespace: "".to_string(),
        }
        //     None => ResponseQuery {
        //         code: 0,
        //         log: "does not exist".to_string(),
        //         info: "".to_string(),
        //         index: 0,
        //         key: request.data,
        //         value: vec![],
        //         proof_ops: None,
        //         height,
        //         codespace: "".to_string(),
        //     },
        // },
        // Err(e) => panic!("Failed to get key \"{}\": {:?}", key, e),
        // }
    }

    fn check_tx(&self, _request: RequestCheckTx) -> ResponseCheckTx {
        ResponseCheckTx {
            code: 0,
            data: vec![],
            log: "".to_string(),
            info: "".to_string(),
            gas_wanted: 1,
            gas_used: 0,
            events: vec![],
            codespace: "".to_string(),
        }
    }

    fn deliver_tx(&self, request: RequestDeliverTx) -> ResponseDeliverTx {
        let tx = serde_cose::from_slice(&request.tx);
        let mut tx1 = cose::sign::CoseSign::new();
        tx1.bytes = request.tx.clone();
        tx1.init_decoder(None).unwrap();
        // tx1.key(&key).unwrap();
        let mut key = cose::keys::CoseKey::new();
        key.kty(cose::keys::EC2);
        key.alg(cose::algs::EDDSA);
        key.crv(cose::keys::ED25519);

        key.x(
            hex::decode("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a")
                .unwrap(),
        );
        key.d(
            hex::decode("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60")
                .unwrap(),
        );
        key.key_ops(vec![cose::keys::KEY_OPS_VERIFY]);
        tx1.key(&key);
        let tx1_result = tx1.decode(None, None);

        ResponseDeliverTx {
            code: 0,
            data: vec![],
            log: format!("{:?}", tx1_result),
            info: format!("{:?}", tx1.header.kid.map(hex::encode)),
            gas_wanted: 0,
            gas_used: 0,
            events: vec![Event {
                r#type: "app".to_string(),
                attributes: vec![
                    // EventAttribute {
                    //     key: "key".as_bytes().to_owned(),
                    //     value: key.as_bytes().to_owned(),
                    //     index: true,
                    // },
                    EventAttribute {
                        key: "index_key".as_bytes().to_owned(),
                        value: "index is working".as_bytes().to_owned(),
                        index: true,
                    },
                    EventAttribute {
                        key: "noindex_key".as_bytes().to_owned(),
                        value: "index is working".as_bytes().to_owned(),
                        index: false,
                    },
                ],
            }],
            codespace: "".to_string(),
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
