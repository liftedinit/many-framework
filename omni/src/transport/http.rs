use crate::message::{RequestMessage, ResponseMessage};
use crate::transport::OmniRequestHandler;
use crate::Identity;
use anyhow::anyhow;
use minicose::{CoseKey, CoseSign1, Ed25519CoseKeyBuilder};
use ring::signature::{Ed25519KeyPair, KeyPair};
use std::io::Cursor;
use std::net::ToSocketAddrs;
use tiny_http::{Request, Response};

/// Maximum of 2MB per HTTP request.
const READ_BUFFER_LEN: usize = 1024 * 1024 * 2;

#[derive(Debug)]
pub struct HttpServer<H: OmniRequestHandler + std::fmt::Debug> {
    handler: H,
    keypair: Option<Ed25519KeyPair>,
    identity: Identity,
}

impl<H: OmniRequestHandler + std::fmt::Debug> HttpServer<H> {
    pub fn new(identity: Identity, keypair: Option<Ed25519KeyPair>, handler: H) -> Self {
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

        Self {
            handler,
            keypair,
            identity,
        }
    }

    async fn execute_handler(&self, request: &RequestMessage) -> ResponseMessage {
        self.handler
            .execute(request)
            .await
            .unwrap_or_else(|err| ResponseMessage::from_request(request, &self.identity, Err(err)))
    }

    async fn handle_request(
        &self,
        request: &mut Request,
        buffer: &mut [u8],
    ) -> Response<std::io::Cursor<Vec<u8>>> {
        match request.body_length() {
            Some(x) if x > READ_BUFFER_LEN => {
                // This is a transport error, and as such an HTTP error.
                return Response::empty(500).with_data(Cursor::new(vec![]), Some(0));
            }
            _ => {}
        }

        let actual_len = match request.as_reader().read(buffer) {
            Ok(x) => x,
            Err(_e) => {
                return Response::empty(500).with_data(Cursor::new(vec![]), Some(0));
            }
        };

        let bytes = &buffer[..actual_len];
        eprintln!(" request: {}", hex::encode(bytes));

        let server_id = &self.identity;
        let envelope = match CoseSign1::from_bytes(bytes) {
            Ok(cs) => cs,
            Err(_e) => {
                return Response::empty(500).with_data(Cursor::new(vec![]), Some(0));
            }
        };

        let response = match crate::message::decode_request_from_cose_sign1(envelope)
            .and_then(|message| self.handler.validate(&message).map(|_| message))
        {
            Ok(message) => self.execute_handler(&message).await,
            Err(err) => ResponseMessage::error(server_id, err),
        };

        let bytes = match crate::message::encode_cose_sign1_from_response(
            response,
            server_id.clone(),
            &self.keypair,
        )
        .and_then(|r| r.to_bytes().map_err(|e| e.to_string()))
        {
            Ok(bytes) => bytes,
            Err(_e) => {
                return Response::empty(500).with_data(Cursor::new(vec![]), Some(0));
            }
        };

        eprintln!("   reply: {}", hex::encode(&bytes));
        Response::from_data(bytes)
    }

    pub fn bind<A: ToSocketAddrs>(&self, addr: A) -> Result<(), anyhow::Error> {
        let mut buffer: Vec<u8> = Vec::new();
        buffer.resize(READ_BUFFER_LEN, 0);
        let server = tiny_http::Server::http(addr).map_err(|e| anyhow!("{}", e))?;

        let runtime = tokio::runtime::Runtime::new().unwrap();

        for mut request in server.incoming_requests() {
            runtime.block_on(async {
                let response = self
                    .handle_request(&mut request, buffer.as_mut_slice())
                    .await;

                // If there's a transport error (e.g. connection closed) on the response itself,
                // we don't actually care and just continue waiting for the next request.
                let _ = request.respond(response);
            });
        }

        Ok(())
    }
}
