use crate::init::AbciInit;
use async_trait::async_trait;
use minicose::CoseSign1;
use omni::identity::cose::CoseKeyIdentity;
use omni::message::{
    decode_response_from_cose_sign1, encode_cose_sign1_from_request, RequestMessageBuilder,
    ResponseMessage,
};
use omni::transport::LowLevelOmniRequestHandler;
use omni::OmniError;
use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};
use tendermint_rpc::Client;

pub struct AbciHttpServer<C: Client> {
    client: C,
    identity: CoseKeyIdentity,
    endpoints: BTreeMap<String, bool>,
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
    pub async fn new(client: C, identity: CoseKeyIdentity) -> Self {
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
        let init_message = AbciInit::from_bytes(&response.data.unwrap()).unwrap();

        Self {
            client,
            identity,
            endpoints: init_message.endpoints,
        }
    }

    async fn execute_inner(&self, envelope: CoseSign1) -> Result<CoseSign1, OmniError> {
        let message = omni::message::decode_request_from_cose_sign1(envelope.clone())?;

        if let Some(is_command) = self.endpoints.get(&message.method) {
            eprintln!("execute inner ({}): \n{:#?}------\n", *is_command, message);
            let data = envelope
                .to_bytes()
                .map_err(|e| OmniError::unexpected_transport_error(e.to_string()))?;

            if *is_command {
                let response = self
                    .client
                    .broadcast_tx_sync(tendermint_rpc::abci::Transaction::from(data))
                    .await
                    .map_err(|e| OmniError::unexpected_transport_error(e.to_string()))?;

                let data = minicbor::to_vec(response.data.value().to_vec())
                    .map_err(|e| OmniError::serialization_error(e.to_string()))?;
                let response =
                    ResponseMessage::from_request(&message, &self.identity.identity, Ok(data));
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
}

#[async_trait]
impl<C: Client + Send + Sync> LowLevelOmniRequestHandler for AbciHttpServer<C> {
    async fn execute(&self, envelope: CoseSign1) -> Result<CoseSign1, String> {
        self.execute_inner(envelope).await.or_else(|err| {
            omni::message::encode_cose_sign1_from_response(
                ResponseMessage::error(&self.identity.identity, err),
                &self.identity,
            )
        })
    }
}
