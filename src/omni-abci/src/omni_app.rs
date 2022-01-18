use crate::module::ABCI_SERVER;
use crate::types::{AbciInit, EndpointInfo};
use async_trait::async_trait;
use minicose::CoseSign1;
use omni::identity::cose::CoseKeyIdentity;
use omni::message::{
    decode_response_from_cose_sign1, encode_cose_sign1_from_request,
    encode_cose_sign1_from_response, RequestMessageBuilder, ResponseMessage,
};
use omni::protocol::{Attribute, Status, StatusBuilder};
use omni::transport::LowLevelOmniRequestHandler;
use omni::OmniError;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Debug, Formatter};
use tendermint_rpc::Client;

pub struct AbciHttpServer<C: Client> {
    client: C,
    identity: CoseKeyIdentity,
    backend_status: Status,
    endpoints: BTreeMap<String, EndpointInfo>,
}

impl<C: Client> Debug for AbciHttpServer<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AbciHttpServer")
            .field("client", &"...")
            .field("identity", &self.identity)
            .finish()
    }
}

impl<C: Client + Send + Sync> AbciHttpServer<C> {
    pub async fn new(client: C, backend_status: Status, identity: CoseKeyIdentity) -> Self {
        let init_message = RequestMessageBuilder::default()
            .from(identity.identity)
            .method("abci.init".to_string())
            .build()
            .unwrap();
        let data = encode_cose_sign1_from_request(init_message, &identity)
            .unwrap()
            .to_bytes()
            .unwrap();

        let response = client.abci_query(None, data, None, false).await.unwrap();
        let response = CoseSign1::from_bytes(&response.value).unwrap();
        let response = decode_response_from_cose_sign1(response, None).unwrap();
        let init_message: AbciInit = minicbor::decode(&response.data.unwrap()).unwrap();

        Self {
            client,
            identity,
            endpoints: init_message.endpoints,
            backend_status,
        }
    }

    fn status(&self) -> Status {
        let attributes: BTreeSet<Attribute> = self
            .backend_status
            .attributes
            .iter()
            .filter(|x| x.id != ABCI_SERVER.id)
            .cloned()
            .collect();

        StatusBuilder::default()
            .name(format!("AbciModule({})", self.backend_status.name))
            .version(1)
            .public_key(self.identity.public_key())
            .identity(self.identity.identity)
            .internal_version(std::env!("CARGO_PKG_VERSION").to_string())
            .attributes(attributes.into_iter().collect())
            .build()
            .unwrap()
    }

    async fn execute_message(&self, envelope: CoseSign1) -> Result<CoseSign1, OmniError> {
        let message = omni::message::decode_request_from_cose_sign1(envelope.clone())?;
        if let Some(info) = self.endpoints.get(&message.method) {
            let is_command = info.should_commit;
            eprintln!("execute inner ({}): \n{:#?}------\n", is_command, message);
            let data = envelope
                .to_bytes()
                .map_err(|e| OmniError::unexpected_transport_error(e.to_string()))?;

            if is_command {
                let response = self
                    .client
                    .broadcast_tx_sync(tendermint_rpc::abci::Transaction::from(data))
                    .await
                    .map_err(|e| OmniError::unexpected_transport_error(e.to_string()))?;

                let _ = minicbor::to_vec(response.data.value().to_vec())
                    .map_err(|e| OmniError::serialization_error(e.to_string()))?;

                // A command will always return an empty payload with an ASYNC attribute.
                let response =
                    ResponseMessage::from_request(&message, &self.identity.identity, Ok(vec![]))
                        .with_attribute(omni::protocol::attributes::response::ASYNC);
                omni::message::encode_cose_sign1_from_response(response, &self.identity)
                    .map_err(OmniError::unexpected_transport_error)
            } else {
                let response = self
                    .client
                    .abci_query(None, data, None, false)
                    .await
                    .map_err(|e| OmniError::unexpected_transport_error(e.to_string()))?;
                eprintln!("bytes: {}", hex::encode(&response.value));
                let response = CoseSign1::from_bytes(&response.value)
                    .map_err(|e| OmniError::unexpected_transport_error(e.to_string()))?;
                Ok(response)
            }
        } else {
            Err(OmniError::invalid_method_name(message.method))
        }
    }

    async fn execute_inner(&self, envelope: CoseSign1) -> Result<CoseSign1, OmniError> {
        let message = omni::message::decode_request_from_cose_sign1(envelope.clone())?;
        if let Some(payload) = match message.method.as_str() {
            "status" => Some(
                self.status()
                    .to_bytes()
                    .map_err(OmniError::serialization_error)?,
            ),
            "heartbeat" => Some(Vec::new()),
            "echo" => Some(message.data.clone()),
            "endpoints" => Some(
                minicbor::to_vec(self.endpoints())
                    .map_err(|e| OmniError::serialization_error(e.to_string()))?,
            ),
            _ => None,
        } {
            let response =
                ResponseMessage::from_request(&message, &self.identity.identity, Ok(payload));
            encode_cose_sign1_from_response(response, &self.identity)
                .map_err(|e| OmniError::unknown(e))
        } else {
            self.execute_message(envelope).await
        }
    }
    fn endpoints(&self) -> Vec<&str> {
        let mut result = vec![
            self.endpoints.keys().map(|x| x.as_str()).collect(),
            vec!["echo", "endpoints", "heartbeat", "status"],
        ]
        .concat();
        result.sort();
        result
    }
}

#[async_trait]
impl<C: Client + Send + Sync> LowLevelOmniRequestHandler for AbciHttpServer<C> {
    async fn execute(&self, envelope: CoseSign1) -> Result<CoseSign1, String> {
        self.execute_inner(envelope).await.or_else(|err| {
            encode_cose_sign1_from_response(
                ResponseMessage::error(&self.identity.identity, err),
                &self.identity,
            )
        })
    }
}
