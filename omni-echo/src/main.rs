use clap::Parser;
use omni::server::module::base::BaseServerModule;
use omni::server::OmniServer;
use omni::transport::http::HttpServer;
use omni::Identity;
use std::path::PathBuf;

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

    let omni = OmniServer::new(id, &keypair);

    HttpServer::simple(id, Some(keypair), omni)
        .bind("0.0.0.0:8001")
        .unwrap();
}
