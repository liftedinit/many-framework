use clap::Parser;
use omni::Identity;
use omni_abci::abci_app::AbciApp;
use omni_abci::omni_app::AbciHttpServer;
use std::net::ToSocketAddrs;
use std::path::PathBuf;
use tendermint_abci::ServerBuilder;
use tracing_subscriber::filter::LevelFilter;

mod abci_app;
mod omni_app;

#[derive(Parser)]
struct Opts {
    /// Address and port to bind the ABCI server to.
    #[clap(long)]
    abci: String,

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

    let abci_app = AbciApp::create(omni_app, Identity::anonymous()).unwrap();

    let ws_url = format!(
        "ws://localhost:{}",
        abci.to_socket_addrs().unwrap().next().unwrap().port()
    );

    let abci_server = ServerBuilder::new(abci_read_buf_size)
        .bind(abci, abci_app)
        .unwrap();
    std::thread::spawn(move || abci_server.listen().unwrap());

    let (abci_client, abci_driver) = tendermint_rpc::WebSocketClient::new(ws_url.as_str())
        .await
        .unwrap();
    tokio::spawn(async move { abci_driver.run().await.unwrap() });
    eprintln!("0");

    let (id, keypair) = Identity::from_pem_addressable(std::fs::read(omni_pem).unwrap()).unwrap();
    let omni_server =
        omni::transport::http::HttpServer::new(AbciHttpServer::new(abci_client, id, Some(keypair)));

    eprintln!("3");
    std::thread::spawn(move || omni_server.bind(omni).unwrap());
}
