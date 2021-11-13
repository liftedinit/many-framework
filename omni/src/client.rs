use crate::message::{
    decode_response_from_cose_sign1, encode_cose_sign1_from_request, RequestMessage,
    RequestMessageBuilder,
};
use crate::protocol::Status;
use crate::{Identity, OmniError};
use minicbor::Encode;
use minicose::CoseSign1;
use reqwest::{IntoUrl, Url};
use ring::signature::Ed25519KeyPair;
use std::convert::TryInto;
use std::fmt::Formatter;
use std::sync::Arc;

#[derive(Clone)]
pub struct OmniClient {
    pub id: Identity,
    keypair: Option<Arc<Ed25519KeyPair>>,
    pub to: Identity,
    url: Url,
}

impl std::fmt::Debug for OmniClient {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OmniClient")
            .field("id", &self.id)
            .field("to", &self.to)
            .field("url", &self.url)
            .finish()
    }
}

impl OmniClient {
    pub fn new<S: IntoUrl, I: TryInto<Identity>>(
        url: S,
        to: Identity,
        identity: I,
        keypair: Option<Ed25519KeyPair>,
    ) -> Result<Self, String> {
        Ok(Self {
            id: identity
                .try_into()
                .map_err(|_e| format!("Could not parse identity."))?,
            keypair: keypair.map(Arc::new),
            to,
            url: url.into_url().map_err(|e| format!("{}", e))?,
        })
    }

    pub fn send_envelope<S: IntoUrl>(url: S, message: CoseSign1) -> Result<CoseSign1, OmniError> {
        let bytes = message
            .to_bytes()
            .map_err(|_| OmniError::internal_server_error())?;

        let client = reqwest::blocking::Client::new();
        let response = client.post(url).body(bytes).send().unwrap();
        let body = response.bytes().unwrap();
        let bytes = body.to_vec();
        CoseSign1::from_bytes(&bytes).map_err(|e| OmniError::deserialization_error(e.to_string()))
    }

    pub fn send_message(&self, message: RequestMessage) -> Result<Vec<u8>, OmniError> {
        let cose = encode_cose_sign1_from_request(
            message,
            self.id.clone(),
            self.keypair.as_ref().map(|x| x.as_ref()),
        )
        .unwrap();
        let cose_sign1 = Self::send_envelope(self.url.clone(), cose)?;

        let response = decode_response_from_cose_sign1(cose_sign1, None)
            .map_err(|e| OmniError::deserialization_error(e))?;

        response.data
    }

    pub fn call_raw<M>(&self, method: M, argument: &[u8]) -> Result<Vec<u8>, OmniError>
    where
        M: Into<String>,
    {
        let message: RequestMessage = RequestMessageBuilder::default()
            .version(1)
            .from(self.id.clone())
            .to(self.to.clone())
            .method(method.into())
            .data(argument.to_vec())
            .build()
            .map_err(|_| OmniError::internal_server_error())?;

        self.send_message(message)
    }

    pub fn call_<M, I>(&self, method: M, argument: I) -> Result<Vec<u8>, OmniError>
    where
        M: Into<String>,
        I: Encode,
    {
        let bytes: Vec<u8> = minicbor::to_vec(argument)
            .map_err(|e| OmniError::serialization_error(e.to_string()))?;

        self.call_raw(method, bytes.as_slice())
    }

    pub fn status(&self) -> Result<Status, OmniError> {
        let response = self.call_("status", ())?;

        let status = minicbor::decode(response.as_slice())
            .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
        Ok(status)
    }
}
