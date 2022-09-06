use async_trait::async_trait;
use coset::{CborSerializable, CoseSign1};
use many_error::ManyError;
use many_identity::CoseKeyIdentity;
use many_modules::abci_backend::{AbciInit, EndpointInfo, ABCI_MODULE_ATTRIBUTE};
use many_modules::base;
use many_protocol::{
    decode_request_from_cose_sign1, decode_response_from_cose_sign1,
    encode_cose_sign1_from_request, encode_cose_sign1_from_response, RequestMessageBuilder,
    ResponseMessage,
};
use many_server::transport::LowLevelManyRequestHandler;
use many_types::attributes::Attribute;
use many_types::cbor::CborAny;
use std::collections::{BTreeMap, BTreeSet};
use std::default::Default;
use std::fmt::{Debug, Formatter};
use tendermint_rpc::Client;

pub struct AbciModuleMany<C: Client> {
    client: C,
    backend_status: base::Status,
    identity: CoseKeyIdentity,
    backend_endpoints: BTreeMap<String, EndpointInfo>,
}

impl<C: Client + Sync> AbciModuleMany<C> {
    pub async fn new(client: C, backend_status: base::Status, identity: CoseKeyIdentity) -> Self {
        let init_message = RequestMessageBuilder::default()
            .from(identity.identity)
            .method("abci.init".to_string())
            .build()
            .unwrap();
        let data = encode_cose_sign1_from_request(init_message, &identity)
            .unwrap()
            .to_vec()
            .unwrap();

        let response = client.abci_query(None, data, None, false).await.unwrap();
        let response = CoseSign1::from_slice(&response.value).unwrap();
        let response = decode_response_from_cose_sign1(response, None).unwrap();
        let init_message: AbciInit = minicbor::decode(&response.data.unwrap()).unwrap();

        Self {
            client,
            backend_status,
            identity,
            backend_endpoints: init_message.endpoints,
        }
    }

    async fn execute_message(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        let message = decode_request_from_cose_sign1(envelope.clone(), None)?;
        if let Some(info) = self.backend_endpoints.get(&message.method) {
            let is_command = info.is_command;
            let data = envelope
                .to_vec()
                .map_err(|e| ManyError::unexpected_transport_error(e.to_string()))?;

            if is_command {
                let response = self
                    .client
                    .broadcast_tx_sync(tendermint_rpc::abci::Transaction::from(data))
                    .await
                    .map_err(ManyError::unexpected_transport_error)?;

                // A command will always return an empty payload with an ASYNC attribute.
                let response =
                    ResponseMessage::from_request(&message, &self.identity.identity, Ok(vec![]))
                        .with_attribute(
                            many_modules::r#async::attributes::ASYNC
                                .with_argument(CborAny::Bytes(response.hash.as_bytes().to_vec())),
                        );
                encode_cose_sign1_from_response(response, &self.identity)
                    .map_err(ManyError::unexpected_transport_error)
            } else {
                let response = self
                    .client
                    .abci_query(None, data, None, false)
                    .await
                    .map_err(|e| ManyError::unexpected_transport_error(e.to_string()))?;

                CoseSign1::from_slice(&response.value)
                    .map_err(|e| ManyError::unexpected_transport_error(e.to_string()))
            }
        } else {
            Err(ManyError::invalid_method_name(message.method))
        }
    }
}

impl<C: Client> Debug for AbciModuleMany<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("AbciModuleFrontend")
    }
}

#[async_trait]
impl<C: Client + Sync + Send> LowLevelManyRequestHandler for AbciModuleMany<C> {
    async fn execute(&self, envelope: CoseSign1) -> Result<CoseSign1, String> {
        let result = self.execute_message(envelope).await;

        match result {
            Ok(x) => Ok(x),
            Err(e) => {
                let response = ResponseMessage::error(&self.identity.identity, None, e);
                encode_cose_sign1_from_response(response, &self.identity)
            }
        }
    }
}

impl<C: Client + Sync + Send> base::BaseModuleBackend for AbciModuleMany<C> {
    fn endpoints(&self) -> Result<base::Endpoints, ManyError> {
        Ok(base::Endpoints(BTreeSet::from_iter(
            self.backend_endpoints.keys().cloned(),
        )))
    }

    fn status(&self) -> Result<base::Status, ManyError> {
        let attributes: BTreeSet<Attribute> = self
            .backend_status
            .attributes
            .iter()
            .filter(|x| x.id != ABCI_MODULE_ATTRIBUTE.id)
            .cloned()
            .collect();

        let mut builder = base::StatusBuilder::default();

        builder
            .name(format!("AbciModule({})", self.backend_status.name))
            .version(1)
            .identity(self.identity.identity)
            .attributes(attributes.into_iter().collect())
            .server_version(std::env!("CARGO_PKG_VERSION").to_string());

        if let Some(pk) = self.identity.public_key() {
            builder.public_key(pk);
        }

        builder
            .build()
            .map_err(|e| ManyError::unknown(e.to_string()))
    }
}
