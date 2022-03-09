use clap::Parser;
use many::server::module::{base, blockchain};
use many::types::identity::cose::CoseKeyIdentity;
use many::{Identity, ManyServer};
use many_client::ManyClient;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tendermint_abci::ServerBuilder;
use tendermint_rpc::Client;
use tracing_subscriber::filter::LevelFilter;

mod abci_app;
mod many_app;
mod module;

use abci_app::AbciApp;
use many_app::AbciModuleMany;
use module::AbciBlockchainModuleImpl;

#[derive(Parser)]
struct Opts {
    /// Address and port to bind the ABCI server to.
    #[clap(long)]
    abci: String,

    /// URL for the tendermint server. Tendermint must already be running.
    #[clap(long)]
    tendermint: String,

    /// URL (including scheme) that has the MANY application running.
    #[clap(long)]
    many_app: String,

    /// Address and port to bind the MANY server to.
    #[clap(long)]
    many: String,

    /// A pem file for the MANY frontend.
    #[clap(long)]
    many_pem: PathBuf,

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
        many_app,
        many,
        many_pem,
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

    // Try to get the status of the backend MANY app.
    let many_client = ManyClient::new(
        &many_app,
        Identity::anonymous(),
        CoseKeyIdentity::anonymous(),
    )
    .unwrap();

    let start = std::time::SystemTime::now();
    eprintln!("Connecting to the backend app...");

    let status = loop {
        let many_client = many_client.clone();
        let result = tokio::task::spawn_blocking(move || many_client.status())
            .await
            .unwrap();

        match result {
            Err(e) => {
                if start.elapsed().unwrap().as_secs() > 60 {
                    tracing::error!(
                        "\nCould not connect to the ABCI server in 60 seconds... Terminating."
                    );
                    tracing::error!(error = e.to_string().as_str());
                    std::process::exit(1);
                }
                tracing::debug!(error = e.to_string().as_str());
                eprint!(".");
            }
            Ok(s) => {
                eprintln!(" Connected.");
                break s;
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(1));
    };

    let abci_app = tokio::task::spawn_blocking(move || {
        AbciApp::create(many_app, Identity::anonymous()).unwrap()
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
    eprintln!("... :1");
    loop {
        let info = abci_client.abci_info().await;
        eprintln!("... :2 {:?}", info);
        if info.is_ok() {
            break;
        }
        if start.elapsed().unwrap().as_secs() > 300 {
            eprintln!("\nCould not connect to the ABCI server in 300 seconds... Terminating.");
            std::process::exit(1);
        }

        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    eprintln!("... :3");

    let key = CoseKeyIdentity::from_pem(&std::fs::read_to_string(&many_pem).unwrap()).unwrap();
    let server = ManyServer::new(format!("AbciModule({})", &status.name), key.clone());
    let backend = AbciModuleMany::new(abci_client.clone(), status, key).await;
    let blockchain_impl = Arc::new(Mutex::new(AbciBlockchainModuleImpl::new(abci_client)));
    eprintln!("... :4");

    {
        let mut s = server.lock().unwrap();
        s.add_module(base::BaseModule::new(server.clone()));
        s.add_module(blockchain::BlockchainModule::new(blockchain_impl));
        s.set_fallback_module(backend);
    }
    eprintln!("... :5");

    let many_server = many::transport::http::HttpServer::new(server);

    tracing::info!("Starting MANY server on addr {}", many.clone());
    let _j_many = std::thread::spawn(move || match many_server.bind(many) {
        Ok(_) => {}
        Err(error) => {
            tracing::error!("{}", error);
            panic!("Error happened in many: {:?}", error);
        }
    });

    j_abci.join().unwrap();
    // When ABCI is done, just kill the whole process.
    // TODO: shutdown the many server gracefully.
    std::process::exit(0);
    // j_many.join().unwrap();
}
