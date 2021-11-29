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

    /// The pem file for the owner of the ledger (who can mint).
    /// By default there are no owners and accounts cannot be minted.
    #[clap(long)]
    owner: Option<PathBuf>,

    /// The port to bind to for the OMNI Http server.
    #[clap(long, short, default_value = "8000")]
    port: u16,

    /// The list of supported symbols.
    #[clap(long, short)]
    symbols: Vec<String>,

    /// Uses an ABCI application module.
    #[clap(long)]
    abci: bool,

    /// Path of a state file (that will be used for the initial setup).
    #[clap(long)]
    state: Option<PathBuf>,
}

fn main() {
    let Opts {
        pem,
        owner,
        port,
        symbols,
        abci,
        state,
    } = Opts::parse();
    let key = CoseKeyIdentity::from_pem(&std::fs::read_to_string(&pem).unwrap()).unwrap();
    let owner_id = owner.map(|owner| {
        CoseKeyIdentity::from_pem(&std::fs::read_to_string(owner).unwrap())
            .unwrap()
            .identity
    });

    let state = state.map_or_else(Default::default, |s| {
        let content = std::fs::read_to_string(&s).unwrap();
        let json: InitialState = serde_json::from_str(&content).unwrap();
        json
    });

    let module = LedgerModule::new(owner_id, symbols, state).unwrap();
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
