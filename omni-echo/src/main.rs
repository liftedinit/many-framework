use async_trait::async_trait;
use clap::Parser;
use minicose::{CoseKey, Ed25519CoseKeyBuilder};
use omni::server::http::Server;
use omni::server::RequestHandler;
use omni::{Identity, OmniError};
use ring::signature::KeyPair;
use std::path::PathBuf;

struct EchoHandler;

#[async_trait]
impl RequestHandler for EchoHandler {
    async fn handle(&self, method: &str, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
        if method == "echo" {
            Ok(payload.to_vec())
        } else {
            Err(OmniError::invalid_method_name(method.to_string()))
        }
    }
}

#[derive(Parser)]
struct Opts {
    /// The location of a Ed25519 PEM file for the identity of this server.
    #[clap(long)]
    pem: PathBuf,
}

fn main() {
    let o: Opts = Opts::parse();
    let bytes = std::fs::read(o.pem).unwrap();
    let content = pem::parse(bytes).unwrap();

    let keypair =
        ring::signature::Ed25519KeyPair::from_pkcs8_maybe_unchecked(&content.contents).unwrap();

    let x = keypair.public_key().as_ref().to_vec();
    let cose_key: CoseKey = Ed25519CoseKeyBuilder::default()
        .x(x)
        .build()
        .unwrap()
        .into();
    let id = Identity::addressable(&cose_key);

    Server::new(EchoHandler, id, Some(keypair))
        .bind("0.0.0.0:8001")
        .unwrap();
}
