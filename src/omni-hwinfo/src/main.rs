use clap::Parser;
use std::path::PathBuf;
use sysinfo::{System, SystemExt};

mod error;
mod module;

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
}

fn main() {
    let Opts {
        verbose: _,
        quiet: _,
        pem: _,
        port: _,
    } = Opts::parse();

    let mut sys = System::new();
    sys.refresh_all();
    eprintln!("... {}", System::IS_SUPPORTED);
    eprintln!("CPU... {}", sys.processors().len());
    for (i, cpu) in sys.processors().iter().enumerate() {
        eprintln!("{} {:?}", i, cpu);
    }
    eprintln!("g: {:?}", sys.global_processor_info());

    eprintln!("---- sys-info ----");
    eprintln!("cpu_speed: {:?}", sys_info::cpu_speed());
    std::process::exit(1);

    // let verbose_level = 2 + verbose - quiet;
    // let log_level = match verbose_level {
    //     x if x > 3 => LevelFilter::TRACE,
    //     3 => LevelFilter::DEBUG,
    //     2 => LevelFilter::INFO,
    //     1 => LevelFilter::WARN,
    //     0 => LevelFilter::ERROR,
    //     x if x < 0 => LevelFilter::OFF,
    //     _ => unreachable!(),
    // };
    // tracing_subscriber::fmt().with_max_level(log_level).init();
    //
    // if clean {
    //     // Delete the persistent storage.
    //     let _ = std::fs::remove_dir_all(persistent.as_path());
    // } else if persistent.exists() {
    //     // Initial state is ignored.
    //     state = None;
    // }
    //
    // let key = CoseKeyIdentity::from_pem(&std::fs::read_to_string(&pem).unwrap()).unwrap();
    //
    // let state = state.map(|state| {
    //     let content = std::fs::read_to_string(&state).unwrap();
    //     serde_json::from_str(&content).unwrap()
    // });
    //
    // let module = if let Some(state) = state {
    //     KvStoreModuleImpl::new(state, persistent, abci).unwrap()
    // } else {
    //     KvStoreModuleImpl::load(persistent, abci).unwrap()
    // };
    //
    // let module = Arc::new(Mutex::new(module));
    //
    // let omni = OmniServer::simple(
    //     "omni-kvstore",
    //     key.clone(),
    //     Some(std::env!("CARGO_PKG_VERSION").to_string()),
    // );
    //
    // {
    //     let mut s = omni.lock().unwrap();
    //     s.add_module(kvstore::KvStoreModule::new(module.clone()));
    //
    //     if abci {
    //         s.add_module(abci_backend::AbciModule::new(module.clone()));
    //     }
    // }
    //
    // HttpServer::new(omni)
    //     .bind(format!("127.0.0.1:{}", port))
    //     .unwrap();
}
