use coset::{CborSerializable, CoseSign1};
use many_client::ManyClient;
use many_error::ManyError;
use many_identity::{Address, CoseKeyIdentity};
use many_modules::abci_backend::{AbciBlock, AbciCommitInfo, AbciInfo};
use many_protocol::ResponseMessage;
use minicbor::Encode;
use reqwest::{IntoUrl, Url};
use std::cell::RefCell;
use std::time::{Duration, SystemTime};
use tendermint_abci::Application;
use tendermint_proto::abci::*;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct AbciApp {
    app_name: String,
    many_client: ManyClient,
    many_url: Url,
    timestamp: RefCell<Option<u64>>,
}

impl AbciApp {
    /// Constructor.
    pub fn create<U>(many_url: U, server_id: Address) -> Result<Self, String>
    where
        U: IntoUrl,
    {
        let many_url = many_url.into_url().map_err(|e| e.to_string())?;

        // TODO: Get the server ID from the many server.
        // let server_id = if server_id.is_anonymous() {
        //     server_id
        // } else {
        //     server_id
        // };

        let many_client =
            ManyClient::new(many_url.clone(), server_id, CoseKeyIdentity::anonymous())?;
        let status = many_client.status().map_err(|x| x.to_string())?;
        let app_name = status.name;

        Ok(Self {
            app_name,
            many_url,
            many_client,
        })
    }

    /// Send an ABCI request to the MANY backend, but use the current block time as the
    /// timestamp for the request.
    fn call<M, I>(&self, method: M, argument: I) -> Result<Vec<u8>, ManyError>
    where
        M: Into<String>,
        I: Encode<()>,
    {
        let bytes: Vec<u8> = minicbor::to_vec(argument)
            .map_err(|e| ManyError::serialization_error(e.to_string()))?;

        let message: many_protocol::RequestMessage =
            many_protocol::RequestMessageBuilder::default()
                .version(1)
                .from(self.many_client.id.identity)
                .to(self.many_client.to)
                .method(method.into())
                .data(bytes.to_vec())
                .timestamp(self.timestamp.borrow().map_or_else(SystemTime::now, |ts| {
                    SystemTime::UNIX_EPOCH
                        .checked_add(Duration::from_secs(ts))
                        .unwrap()
                }))
                .build()
                .map_err(|_| ManyError::internal_server_error())?;

        self.many_client.send_message(message)?.data
    }
}

impl Application for AbciApp {
    fn info(&self, request: RequestInfo) -> ResponseInfo {
        debug!(
            "Got info request. Tendermint version: {}; Block version: {}; P2P version: {}",
            request.version, request.block_version, request.p2p_version
        );

        let AbciInfo { height, hash } = match self.call("abci.info", ()).and_then(|payload| {
            minicbor::decode(&payload).map_err(|e| ManyError::deserialization_error(e.to_string()))
        }) {
            Ok(x) => x,
            Err(err) => {
                return ResponseInfo {
                    data: format!("An error occurred during call to abci.info:\n{}", err),
                    ..Default::default()
                }
            }
        };

        ResponseInfo {
            data: format!("many-abci-bridge({})", self.app_name),
            version: env!("CARGO_PKG_VERSION").to_string(),
            app_version: 1,
            last_block_height: height as i64,
            last_block_app_hash: hash.to_vec().into(),
        }
    }
    fn init_chain(&self, _request: RequestInitChain) -> ResponseInitChain {
        Default::default()
    }
    fn query(&self, request: RequestQuery) -> ResponseQuery {
        let cose = match CoseSign1::from_slice(&request.data) {
            Ok(x) => x,
            Err(err) => {
                return ResponseQuery {
                    code: 2,
                    log: err.to_string(),
                    ..Default::default()
                }
            }
        };
        let value = match ManyClient::send_envelope(self.many_url.clone(), cose) {
            Ok(cose_sign) => cose_sign,

            Err(err) => {
                return ResponseQuery {
                    code: 3,
                    log: err.to_string(),
                    ..Default::default()
                }
            }
        };

        match value.to_vec() {
            Ok(value) => ResponseQuery {
                code: 0,
                value: value.into(),
                ..Default::default()
            },
            Err(err) => ResponseQuery {
                code: 1,
                log: err.to_string(),
                ..Default::default()
            },
        }
    }

    fn begin_block(&self, request: RequestBeginBlock) -> ResponseBeginBlock {
        let time = request
            .header
            .and_then(|x| x.time.map(|x| x.seconds as u64));

        let block = AbciBlock { time };
        self.timestamp.replace(time);

        let _ = self.call("abci.beginBlock", block);
        ResponseBeginBlock { events: vec![] }
    }

    fn deliver_tx(&self, request: RequestDeliverTx) -> ResponseDeliverTx {
        let cose = match CoseSign1::from_slice(&request.tx) {
            Ok(x) => x,
            Err(err) => {
                return ResponseDeliverTx {
                    code: 2,
                    log: err.to_string(),
                    ..Default::default()
                }
            }
        };
        match ManyClient::send_envelope(self.many_url.clone(), cose) {
            Ok(cose_sign) => {
                let payload = cose_sign.payload.unwrap_or_default();
                let mut response = ResponseMessage::from_bytes(&payload).unwrap_or_default();

                // Consensus will sign the result, so the `from` field is unnecessary.
                response.from = Address::anonymous();
                // The version is ignored and removed.
                response.version = None;
                // The timestamp MIGHT differ between two nodes so we just force it to be 0.
                response.timestamp = Some(SystemTime::UNIX_EPOCH);

                if let Ok(data) = response.to_bytes() {
                    ResponseDeliverTx {
                        code: 0,
                        data: data.into(),
                        ..Default::default()
                    }
                } else {
                    ResponseDeliverTx {
                        code: 3,
                        ..Default::default()
                    }
                }
            }
            Err(err) => ResponseDeliverTx {
                code: 1,
                data: vec![].into(),
                log: err.to_string(),
                ..Default::default()
            },
        }
    }

    fn end_block(&self, _request: RequestEndBlock) -> ResponseEndBlock {
        let _ = self.call("abci.endBlock", ());
        Default::default()
    }

    fn flush(&self) -> ResponseFlush {
        Default::default()
    }

    fn commit(&self) -> ResponseCommit {
        self.call("abci.commit", ()).map_or_else(
            |err| ResponseCommit {
                data: err.to_string().into_bytes().into(),
                retain_height: 0,
            },
            |msg| {
                let info: AbciCommitInfo = minicbor::decode(&msg).unwrap();
                ResponseCommit {
                    data: info.hash.to_vec().into(),
                    retain_height: info.retain_height as i64,
                }
            },
        )
    }
}
