use clap::Parser;
use omni::identity::cose::CoseKeyIdentity;
use omni::server::OmniServer;
use omni::transport::http::HttpServer;
use std::path::PathBuf;
use tracing::level_filters::LevelFilter;

mod error;
mod module;
mod storage;
mod utils;

use module::*;

#[derive(Parser)]
struct Opts {
    /// Increase output logging verbosity to DEBUG level.
    #[clap(short, long, parse(from_occurrences))]
    verbose: i8,

    /// Suppress all output logging. Can be used multiple times to suppress more.
    #[clap(short, long, parse(from_occurrences))]
    quiet: i8,

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
    state: Option<PathBuf>,

    /// Path to a persistent store database (rocksdb).
    #[clap(long)]
    persistent: PathBuf,

    /// Delete the persistent storage to start from a clean state.
    /// If this is not specified the initial state will not be used.
    #[clap(long, short)]
    clean: bool,
}

fn main() {
    let Opts {
        verbose,
        quiet,
        pem,
        port,
        abci,
        mut state,
        persistent,
        clean,
    } = Opts::parse();

    let verbose_level = 2 + verbose - quiet;
    let log_level = match verbose_level {
        x if x > 3 => LevelFilter::TRACE,
        3 => LevelFilter::DEBUG,
        2 => LevelFilter::INFO,
        1 => LevelFilter::WARN,
        0 => LevelFilter::ERROR,
        x if x < 0 => LevelFilter::OFF,
        _ => unreachable!(),
    };
    tracing_subscriber::fmt().with_max_level(log_level).init();

    if clean {
        // Delete the persistent storage.
        let _ = std::fs::remove_dir_all(persistent.as_path());
    } else if persistent.exists() {
        // Initial state is ignored.
        state = None;
    }

    let key = CoseKeyIdentity::from_pem(&std::fs::read_to_string(&pem).unwrap()).unwrap();

    let state = state.map(|state| {
        let content = std::fs::read_to_string(&state).unwrap();
        serde_json::from_str(&content).unwrap()
    });

    let module = if let Some(state) = state {
        KvStoreModule::new(state, persistent, abci).unwrap()
    } else {
        KvStoreModule::load(persistent, abci).unwrap()
    };
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