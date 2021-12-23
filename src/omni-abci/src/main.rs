use clap::Parser;
use omni::identity::cose::CoseKeyIdentity;
use omni::{Identity, OmniClient};
use std::path::PathBuf;
use tendermint_abci::ServerBuilder;
use tendermint_rpc::Client;
use tracing_subscriber::filter::LevelFilter;

mod abci_app;
mod omni_app;
mod types;

use abci_app::AbciApp;
use omni_app::AbciHttpServer;

#[derive(Parser)]
struct Opts {
    /// Address and port to bind the ABCI server to.
    #[clap(long)]
    abci: String,

    /// URL for the tendermint server. Tendermint must already be running.
    #[clap(long)]
    tendermint: String,

    /// URL (including scheme) that has the OMNI application running.
    #[clap(long)]
    omni_app: String,

    /// Address and port to bind the OMNI server to.
    #[clap(long)]
    omni: String,

    /// A pem file for the OMNI frontend.
    #[clap(long)]
    omni_pem: PathBuf,

    /// The default server read buffer size, in bytes, for each incoming client connection.
    #[clap(short, long, default_value = "1048576")]
    abci_read_buf_size: usize,

    /// Increase output logging verbosity to DEBUG level.
    #[clap(short, long, parse(from_occurrences))]
    verbose: i8,

    /// Suppress all output logging. Can be used multiple times to suppress more.
    #[clap(short, long, parse(from_occurrences))]
    quiet: i8,
}

#[tokio::main]
async fn main() {
    let Opts {
        abci,
        tendermint,
        omni_app,
        omni,
        omni_pem,
        abci_read_buf_size,
        verbose,
        quiet,
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

    // Try to get the status of the backend OMNI app.
    let omni_client = OmniClient::new(
        &omni_app,
        Identity::anonymous(),
        CoseKeyIdentity::anonymous(),
    )
    .unwrap();

    let start = std::time::SystemTime::now();
    eprintln!("Connecting to the backend app...");

    loop {
        let omni_client = omni_client.clone();
        let result = tokio::task::spawn_blocking(move || omni_client.status())
            .await
            .unwrap();
        if let Err(e) = result {
            if start.elapsed().unwrap().as_secs() > 60 {
                eprintln!("\nCould not connect to the ABCI server in 60 seconds... Terminating.");
                eprintln!("Latest error:\n{}\n", e);
                std::process::exit(1);
            }
            eprint!(".");
        } else {
            eprintln!(" Connected.");
            break;
        }

        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    let abci_app = tokio::task::spawn_blocking(move || {
        AbciApp::create(omni_app, Identity::anonymous()).unwrap()
    })
    .await
    .unwrap();

    let abci_server = ServerBuilder::new(abci_read_buf_size)
        .bind(abci, abci_app)
        .unwrap();
    let j_abci = std::thread::spawn(move || abci_server.listen().unwrap());

    let abci_client = tendermint_rpc::HttpClient::new(tendermint.as_str()).unwrap();

    // Wait for 60 seconds until we can contact the ABCI server.
    let start = std::time::SystemTime::now();
    loop {
        if abci_client.abci_info().await.is_ok() {
            break;
        }
        if start.elapsed().unwrap().as_secs() > 60 {
            eprintln!("\nCould not connect to the ABCI server in 60 seconds... Terminating.");
            std::process::exit(1);
        }

        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    let key = CoseKeyIdentity::from_pem(&std::fs::read_to_string(&omni_pem).unwrap()).unwrap();
    let omni_server =
        omni::transport::http::HttpServer::new(AbciHttpServer::new(abci_client, key).await);

    let _j_omni = std::thread::spawn(move || omni_server.bind(omni).unwrap());

    j_abci.join().unwrap();
    // When ABCI is done, just kill the whole process.
    // TODO: shutdown the omni server gracefully.
    std::process::exit(0);
    // j_omni.join().unwrap();
}
