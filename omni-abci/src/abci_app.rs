use omni::message::RequestMessage;
use omni::{Identity, OmniClient, OmniError};
use reqwest::{IntoUrl, Url};
use std::ops::Shl;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use tendermint_abci::Application;
use tendermint_proto::abci::{
    RequestCheckTx, RequestDeliverTx, RequestInfo, RequestQuery, ResponseCheckTx, ResponseCommit,
    ResponseDeliverTx, ResponseInfo, ResponseQuery,
};
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

        let (last_block_height, last_block_app_hash) = match self.omni_client.call_("abci.info", ())
        {
            Ok(payload) => (0, Vec::new()),
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
            last_block_height: last_block_height as i64,
            last_block_app_hash: last_block_app_hash.to_vec(),
        }
    }
}
