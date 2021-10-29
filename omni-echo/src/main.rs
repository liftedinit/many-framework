use async_trait::async_trait;
use clap::Parser;
use omni::server::function::FunctionMapRequestHandler;
use omni::server::OmniServer;
use omni::transport::http::HttpServer;
use omni::transport::{SimpleRequestHandler, SimpleRequestHandlerAdapter};
use omni::{Identity, OmniError};
use std::path::PathBuf;

struct EchoHandler;

fn echo(payload: &[u8]) -> Result<Vec<u8>, OmniError> {
    Ok(payload.to_vec())
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

    let omni = OmniServer::new(id, &keypair).with_namespace(
        "echo",
        FunctionMapRequestHandler::empty().with_method("echo", echo),
    );

    HttpServer::new(id, Some(keypair), omni)
        .bind("0.0.0.0:8001")
        .unwrap();
}
