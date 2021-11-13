use crate::module::AbciInfo;
use minicose::CoseSign1;
use omni::{Identity, OmniClient, OmniError};
use reqwest::{IntoUrl, Url};
use tendermint_abci::Application;
use tendermint_proto::abci::*;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct AbciApp {
    omni_client: OmniClient,
    omni_url: Url,
}

impl AbciApp {
    /// Constructor.
    pub fn create<U>(omni_url: U, server_id: Identity) -> Result<Self, String>
    where
        U: IntoUrl,
    {
        let omni_url = omni_url.into_url().map_err(|e| e.to_string())?;

        let server_id = if server_id.is_anonymous() {
            // TODO: Get the server ID from the omni server.
            server_id
        } else {
            server_id
        };

        Ok(Self {
            omni_url: omni_url.clone(),
            omni_client: OmniClient::new(omni_url.clone(), server_id, Identity::anonymous(), None)
                .map_err(|e| e.to_string())?,
        })
    }
}

impl Application for AbciApp {
    fn info(&self, request: RequestInfo) -> ResponseInfo {
        debug!(
            "Got info request. Tendermint version: {}; Block version: {}; P2P version: {}",
            request.version, request.block_version, request.p2p_version
        );

        let status = match self.omni_client.status() {
            Ok(status) => status,
            Err(e) => {
                return ResponseInfo {
                    data: format!("An error occurred during call to status:\n{}", e),
                    ..Default::default()
                }
            }
        };

        let AbciInfo { height, hash } =
            match self.omni_client.call_("abci.info", ()).and_then(|payload| {
                minicbor::decode(&payload)
                    .map_err(|e| OmniError::deserialization_error(e.to_string()))
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
            data: format!("omni-abci-bridge({})", status.name),
            version: env!("CARGO_PKG_VERSION").to_string(),
            app_version: 1,
            last_block_height: height as i64,
            last_block_app_hash: hash,
        }
    }

    fn commit(&self) -> ResponseCommit {
        self.omni_client.call_("abci.commit", ()).map_or_else(
            |err| ResponseCommit {
                data: err.to_string().into_bytes(),
                retain_height: 0,
            },
            |_msg| ResponseCommit {
                data: vec![],
                retain_height: 0,
            },
        )
    }

    fn query(&self, request: RequestQuery) -> ResponseQuery {
        let cose = match CoseSign1::from_bytes(&request.data) {
            Ok(x) => x,
            Err(err) => {
                return ResponseQuery {
                    code: 2,
                    log: err.to_string(),
                    ..Default::default()
                }
            }
        };
        let value = match OmniClient::send_envelope(self.omni_url.clone(), cose) {
            Ok(cose_sign) => cose_sign,

            Err(err) => {
                return ResponseQuery {
                    code: 3,
                    log: err.to_string(),
                    ..Default::default()
                }
            }
        };
        match value.to_bytes() {
            Ok(value) => ResponseQuery {
                code: 0,
                value,
                ..Default::default()
            },
            Err(err) => ResponseQuery {
                code: 1,
                log: err.to_string(),
                ..Default::default()
            },
        }
    }

    fn deliver_tx(&self, request: RequestDeliverTx) -> ResponseDeliverTx {
        let cose = match CoseSign1::from_bytes(&request.tx) {
            Ok(x) => x,
            Err(err) => {
                return ResponseDeliverTx {
                    code: 2,
                    log: err.to_string(),
                    ..Default::default()
                }
            }
        };
        match OmniClient::send_envelope(self.omni_url.clone(), cose) {
            Ok(cose_sign) => ResponseDeliverTx {
                code: 0,
                data: cose_sign.payload.unwrap_or_default(),
                ..Default::default()
            },
            Err(err) => ResponseDeliverTx {
                code: 1,
                data: vec![],
                log: err.to_string(),
                ..Default::default()
            },
        }
    }
}
