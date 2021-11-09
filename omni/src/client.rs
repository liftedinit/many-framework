use crate::message::send_raw;
use crate::protocol::Status;
use crate::{Identity, OmniError};
use minicbor::{Encode, Encoder};
use reqwest::{IntoUrl, Url};
use ring::signature::Ed25519KeyPair;
use std::convert::TryInto;

pub struct OmniClient {
    id: Identity,
    keypair: Option<Ed25519KeyPair>,
    to: Identity,
    url: Url,
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
            keypair,
            to,
            url: url.into_url().map_err(|e| format!("{}", e))?,
        })
    }

    pub fn call_<M, I>(&self, method: M, argument: I) -> Result<Vec<u8>, OmniError>
    where
        M: Into<String>,
        I: Encode,
    {
        let mut bytes: Vec<u8> = Vec::new();
        let mut e = Encoder::new(&mut bytes);
        argument
            .encode(&mut e)
            .map_err(|e| OmniError::serialization_error(e.to_string()))?;

        let from_identity = self.id.clone();

        send_raw(
            self.url.clone(),
            self.keypair.as_ref().map(|kp| (from_identity, kp)),
            self.to.clone(),
            method.into(),
            bytes,
        )
    }

    pub fn status(&self) -> Result<Status, OmniError> {
        let response = self.call_("status", ())?;

        let status = minicbor::decode(response.as_slice())
            .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
        Ok(status)
    }
}
