use crate::cbor::cose::CoseSign1;
use crate::cbor::message::{RequestMessage, ResponseMessage, ResponseMessageBuilder};
use crate::cbor::value::CborValue;
use crate::server::RequestHandler;
use crate::Identity;
use anyhow::anyhow;
use std::convert::TryFrom;
use std::io::Cursor;
use std::net::ToSocketAddrs;
use tiny_http::{Request, Response, StatusCode};

const READ_BUFFER_LEN: usize = 1024 * 1024 * 10;

fn from_der(der: &[u8]) -> Result<Vec<u8>, String> {
    use simple_asn1::{
        from_der, oid,
        ASN1Block::{BitString, ObjectIdentifier, Sequence},
    };

    let object = from_der(der).map_err(|e| format!("asn error: {:?}", e))?;
    let first = object.first().ok_or(format!("empty object"))?;

    match first {
        Sequence(_, blocks) => {
            let algorithm = blocks.get(0).ok_or(format!("Invalid ASN1"))?;
            let bytes = blocks.get(1).ok_or(format!("Invalid ASN1"))?;
            let id_ed25519 = oid!(1, 3, 101, 112);
            match (algorithm, bytes) {
                (Sequence(_, oid_sequence), BitString(_, _, bytes)) => match oid_sequence.first() {
                    Some(ObjectIdentifier(_, oid)) => {
                        if oid == id_ed25519 {
                            Ok(bytes.clone())
                        } else {
                            Err(format!("Invalid oid."))
                        }
                    }
                    _ => Err(format!("Invalid oid.")),
                },
                _ => Err(format!("Invalid oid.")),
            }
        }
        _ => Err(format!("Invalid root type."))?,
    }
}

pub struct Server<H: RequestHandler> {
    handler: H,
}

impl<H: RequestHandler> Server<H> {
    pub fn new(handler: H) -> Self {
        Self { handler }
    }

    fn get_key_for_identity(
        &self,
        cose_sign1: &CoseSign1,
        kid: Vec<u8>,
    ) -> Option<ring::signature::UnparsedPublicKey<Vec<u8>>> {
        let v = cose_sign1
            .protected
            .custom_headers
            .get(&CborValue::TextString("keys".to_string()))?;

        let key_bytes = match v {
            CborValue::Map(ref m) => {
                let value = m.get(&CborValue::ByteString(kid.clone()))?;
                match value {
                    CborValue::ByteString(value) => Some(value),
                    _ => None,
                }
            }
            _ => None,
        }?;

        // Verify the keybytes matches the identity.
        let id = Identity::try_from(kid.as_slice()).ok()?;
        if id.is_anonymous() {
            return None;
        } else if id.is_public_key() {
            let other = Identity::public_key(key_bytes.to_vec());
            if other == id {
                Some(ring::signature::UnparsedPublicKey::new(
                    &ring::signature::ED25519,
                    from_der(key_bytes).ok()?,
                ))
            } else {
                None
            }
        } else if id.is_addressable() {
            if Identity::addressable(key_bytes.to_vec()) == id {
                Some(ring::signature::UnparsedPublicKey::new(
                    &ring::signature::ED25519,
                    key_bytes.to_owned(),
                ))
            } else {
                None
            }
        } else {
            None
        }
    }

    // TODO: add verification of the `to` fields.
    fn verify(&self, cose_sign1: &CoseSign1) -> bool {
        if let Some(ref kid) = cose_sign1.protected.key_identifier {
            if let Ok(id) = Identity::from_bytes(kid) {
                if id.is_anonymous() {
                    // TODO: allow anonymous requests IF THEY MATCH the message's from field.
                    return false;
                }
            }

            self.get_key_for_identity(cose_sign1, kid.clone())
                .map(|key| {
                    cose_sign1
                        .verify_with(|content, sig| key.verify(content, sig).is_ok())
                        .unwrap_or(false)
                })
                .unwrap_or(false)
        } else {
            false
        }
    }

    fn decode_and_verify(&self, bytes: &[u8]) -> Result<RequestMessage, String> {
        let cose_sign1 = minicbor::decode::<CoseSign1>(bytes)
            .map_err(|e| format!("Invalid COSE CBOR message: {}", e))?;

        if !self.verify(&cose_sign1) {
            return Err("Could not verify the signature.".to_string());
        }

        if let Some(payload) = cose_sign1.payload {
            let mut message = RequestMessage::from_bytes(&payload)?;

            // Update `from` and `to` if they're missing.
            message.from = match message.from {
                None => Some(
                    Identity::from_bytes(&cose_sign1.protected.key_identifier.unwrap_or_default())
                        .map_err(|e| format!("{:?}", e))?,
                ),
                Some(from) => Some(from),
            };

            // TODO: add `to` overload with the threshold key from this blockchain.

            Ok(message)
        } else {
            Err("payload missing".to_string())
        }
    }

    fn encode_and_sign(
        &self,
        public_key: Option<Vec<u8>>,
        response: ResponseMessage,
    ) -> Result<CoseSign1, String> {
        response.to_cose(public_key.map(|pk| (pk, |bytes: &[u8]| self.handler.sign(bytes))))
    }

    fn handle_request(
        &self,
        request: &mut Request,
        buffer: &mut [u8],
    ) -> Result<Response<std::io::Cursor<Vec<u8>>>, anyhow::Error> {
        match request.body_length() {
            Some(x) if x > READ_BUFFER_LEN => {
                return Err(anyhow!("body too long"));
            }
            _ => {}
        }

        let actual_len = request.as_reader().read(buffer)?;
        let bytes = &buffer[..actual_len];
        eprintln!("  bytes: {}", hex::encode(bytes));

        let message = self
            .decode_and_verify(bytes)
            .map_err(|e| anyhow!("{}", e))?;

        let RequestMessage {
            method,
            data,
            to,
            id,
            ..
        } = message;

        let public_key = self.handler.public_key();
        let mut response_builder = ResponseMessageBuilder::default();
        response_builder.version(1).from(
            public_key
                .clone()
                .map_or(Identity::anonymous(), Identity::addressable),
        );

        if let Some(to) = to {
            response_builder.to(to);
        }
        if let Some(id) = id {
            response_builder.id(id);
        }
        match self.handler.handle(method, data) {
            Ok(Some(data)) => {
                response_builder.data(Ok(data));
            }
            Ok(None) => {}
            Err(err) => {
                response_builder.data(Err(err));
            }
        };

        let response = response_builder.build()?;

        let cose_sign1 = self
            .encode_and_sign(public_key, response)
            .map_err(|e| anyhow!("{}", e))?;
        let bytes = cose_sign1.encode().map_err(|e| anyhow!("{}", e))?;

        eprintln!("  reply: {}", hex::encode(&bytes));
        Ok(Response::from_data(bytes))
    }

    pub fn bind<A: ToSocketAddrs>(&self, addr: A) -> Result<(), anyhow::Error> {
        let mut buffer: Vec<u8> = Vec::new();
        buffer.resize(READ_BUFFER_LEN, 0);
        let server = tiny_http::Server::http(addr).map_err(|e| anyhow!("{}", e))?;

        for mut request in server.incoming_requests() {
            eprintln!("request: {:?}", &request);

            let response = self
                .handle_request(&mut request, buffer.as_mut_slice())
                .unwrap_or_else(|err| {
                    eprintln!("err: {:?}", err);
                    Response::new(StatusCode(500), vec![], Cursor::default(), None, None)
                });

            // If there's a transport error (e.g. connection closed) on the response itself,
            // we don't actually care and just continue waiting for the next request.
            let _ = request.respond(response);
        }

        Ok(())
    }
}
