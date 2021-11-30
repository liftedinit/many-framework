use clap::Parser;
use omni::identity::cose::CoseKeyIdentity;
use omni::server::OmniServer;
use omni::transport::http::HttpServer;
use std::path::PathBuf;

mod error;
mod module;
mod storage;

use module::*;
use storage::*;

#[derive(Parser)]
struct Opts {
    /// The location of a PEM file for the identity of this server.
    #[clap(long)]
    pem: PathBuf,

    /// The port to bind to for the OMNI Http server.
    #[clap(long, short, default_value = "8000")]
    port: u16,

    /// Uses an ABCI application module.
    #[clap(long)]
    abci: bool,

    /// Path of a state file (that will be used for the initial setup).
    #[clap(long)]
    state: PathBuf,
}

fn main() {
    let Opts {
        pem,
        port,
        abci,
        state,
    } = Opts::parse();
    let key = CoseKeyIdentity::from_pem(&std::fs::read_to_string(&pem).unwrap()).unwrap();

    let content = std::fs::read_to_string(&state).unwrap();
    let state: InitialState = serde_json::from_str(&content).unwrap();

    let module = LedgerModule::new(state).unwrap();
    let omni = OmniServer::new("omni-ledger", key.clone());
    let omni = if abci {
        omni.with_module(omni_abci::module::AbciModule::new(module))
    } else {
        omni.with_module(module)
    };

    HttpServer::simple(key, omni)
        .bind(format!("127.0.0.1:{}", port))
        .unwrap();
}
