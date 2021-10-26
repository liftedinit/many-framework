use async_trait::async_trait;
use clap::Parser;
use omni::server::http::Server;
use omni::server::RequestHandler;
use omni::{Identity, OmniError};
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
    let (id, keypair) = Identity::from_pem_addressable(bytes).unwrap();

    Server::new(EchoHandler, id, Some(keypair))
        .bind("0.0.0.0:8001")
        .unwrap();
}
