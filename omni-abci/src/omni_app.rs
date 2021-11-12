use crate::module::AbciInit;
use async_trait::async_trait;
use minicose::{CoseKey, CoseSign1, Ed25519CoseKeyBuilder};
use omni::message::{
    decode_response_from_cose_sign1, encode_cose_sign1_from_request, RequestMessage,
    RequestMessageBuilder, ResponseMessage,
};
use omni::transport::LowLevelOmniRequestHandler;
use omni::{Identity, OmniError};
use ring::signature::{Ed25519KeyPair, KeyPair};
use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use tendermint_rpc::Client;

pub struct AbciHttpServer<C: Client> {
    client: C,
    identity: Identity,
    keypair: Option<Ed25519KeyPair>,
    endpoints: BTreeMap<String, bool>,
}

impl<C: Client> Debug for AbciHttpServer<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AbciHttpServer")
            .field("client", &"...")
            .field("identity", &self.identity)
            .field("keypair", &"...")
            .finish()
    }
}

impl<C: Client + Send + Sync> AbciHttpServer<C> {
    pub async fn new(client: C, identity: Identity, keypair: Option<Ed25519KeyPair>) -> Self {
        let cose_key: Option<CoseKey> = keypair.as_ref().map(|kp| {
            let x = kp.public_key().as_ref().to_vec();
            Ed25519CoseKeyBuilder::default()
                .x(x)
                .kid(identity.to_vec())
                .build()
                .unwrap()
                .into()
        });

        assert!(
            identity.matches_key(&cose_key),
            "Identity does not match keypair."
        );
        assert!(identity.is_addressable(), "Identity is not addressable.");

        let init_message = RequestMessageBuilder::default()
            .from(identity.clone())
            .method("abci.init".to_string())
            .build()
            .unwrap();
        let data = encode_cose_sign1_from_request(init_message, identity.clone(), keypair.as_ref())
            .unwrap()
            .to_bytes()
            .unwrap();

        let response = client.abci_query(None, data, None, false).await.unwrap();
        eprintln!("{:?}", response);
        let response = CoseSign1::from_bytes(&response.value).unwrap();
        let response = decode_response_from_cose_sign1(response, None).unwrap();
        let init_message = AbciInit::from_bytes(&response.data.unwrap()).unwrap();

        Self {
            client,
            identity,
            keypair,
            endpoints: init_message.endpoints,
        }
    }

    async fn execute_inner(&self, envelope: CoseSign1) -> Result<ResponseMessage, OmniError> {
        let message = omni::message::decode_request_from_cose_sign1(envelope.clone())?;

        if let Some(is_command) = self.endpoints.get(&message.method) {
            let payload = if *is_command {
                let response = self
                    .client
                    .broadcast_tx_async(tendermint::abci::Transaction::from(
                        envelope
                            .to_bytes()
                            .map_err(|_| OmniError::internal_server_error())?,
                    ))
                    .await
                    .map_err(|_| OmniError::internal_server_error())?;
                response.data.value().to_vec()
            } else {
                let response = self
                    .client
                    .abci_query(
                        None,
                        envelope
                            .to_bytes()
                            .map_err(|_| OmniError::internal_server_error())?,
                        None,
                        false,
                    )
                    .await
                    .map_err(|_| OmniError::internal_server_error())?;
                return Ok(ResponseMessage::from_bytes(&response.value)
                    .map_err(|_| OmniError::internal_server_error())?);
            };

            Ok(ResponseMessage::from_request(
                &message,
                &self.identity,
                Ok(payload),
            ))
        } else {
            Err(OmniError::invalid_method_name(message.method))
        }
    }
}

#[async_trait]
impl<C: Client + Send + Sync> LowLevelOmniRequestHandler for AbciHttpServer<C> {
    async fn execute(&self, envelope: CoseSign1) -> Result<CoseSign1, String> {
        let response = self
            .execute_inner(envelope)
            .await
            .unwrap_or_else(|err| ResponseMessage::error(&self.identity, err));

        omni::message::encode_cose_sign1_from_response(
            response,
            self.identity.clone(),
            self.keypair.as_ref(),
        )
    }
}
