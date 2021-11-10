use clap::Parser;
use omni::server::OmniServer;
use omni::transport::http::HttpServer;
use omni::Identity;
use std::path::PathBuf;

#[derive(Parser)]
struct Opts {
    /// The location of a Ed25519 PEM file for the identity of this server.
    #[clap(long)]
    pem: PathBuf,

    /// The port to bind to, locally.
    #[clap(long, default_value = "8000")]
    port: u16,
}

fn main() {
    let o: Opts = Opts::parse();
    let bytes = std::fs::read(o.pem).unwrap();
    let (id, keypair) = Identity::from_pem_addressable(bytes).unwrap();

    let omni = OmniServer::new("echo", id, &keypair);

    HttpServer::simple(id, Some(keypair), omni)
        .bind(format!("127.0.0.1:{}", o.port))
        .unwrap();
}
