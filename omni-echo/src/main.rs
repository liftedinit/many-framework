use clap::Parser;
use omni::identity::cose::CoseKeyIdentity;
use omni::server::OmniServer;
use omni::transport::http::HttpServer;
use std::path::PathBuf;

#[derive(Parser)]
struct Opts {
    /// The location of a PEM file for the identity of this server.
    #[clap(long)]
    pem: PathBuf,

    /// The port to bind to, locally.
    #[clap(long, default_value = "8000")]
    port: u16,
}

fn main() {
    let o: Opts = Opts::parse();
    let bytes = std::fs::read(o.pem).unwrap();
    let id = CoseKeyIdentity::from_pem(&String::from_utf8(bytes).unwrap()).unwrap();

    let omni = OmniServer::new("echo", id.clone());

    HttpServer::simple(id, omni)
        .bind(format!("127.0.0.1:{}", o.port))
        .unwrap();
}
