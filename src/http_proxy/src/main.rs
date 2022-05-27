use clap::Parser;
use many::server::module::kvstore::{GetArgs, GetReturns};
use many::types::identity::cose::CoseKeyIdentity;
use many::Identity;
use many_client::ManyClient;
use std::net::SocketAddr;
use std::path::PathBuf;
use tiny_http::{Header, Method, Response, StatusCode};
use tracing::warn;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;

#[derive(Parser)]
struct Opts {
    /// Many server URL to connect to. It must implement a KV-Store attribute.
    #[clap(default_value = "http://localhost:8000")]
    server: String,

    /// Port and address to bind to.
    #[clap(long)]
    addr: SocketAddr,

    /// The identity of the server (an identity string), or anonymous if you don't know it.
    server_id: Option<Identity>,

    /// A PEM file for the identity. If not specified, anonymous will be used.
    #[clap(long)]
    pem: Option<PathBuf>,

    /// Increase output logging verbosity to DEBUG level.
    #[clap(short, long, parse(from_occurrences))]
    verbose: i8,

    /// Suppress all output logging. Can be used multiple times to suppress more.
    #[clap(short, long, parse(from_occurrences))]
    quiet: i8,
}

fn main() {
    let Opts {
        addr,
        pem,
        server,
        server_id,
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
    let identity = std::ffi::CStr::from_bytes_with_nul(b"http_proxy\0").unwrap();
    let (options, facility) = Default::default();
    let syslog = tracing_syslog::Syslog::new(identity, options, facility).unwrap();
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt::Subscriber::builder()
            .with_max_level(log_level)
            .with_writer(syslog)
            .finish()
            .with(tracing_subscriber::fmt::Layer::default().with_writer(std::io::stdout)),
    )
    .expect("Unable to set global tracing subscriber");

    let server_id = server_id.unwrap_or_default();
    let key = pem.map_or_else(CoseKeyIdentity::anonymous, |p| {
        CoseKeyIdentity::from_pem(&std::fs::read_to_string(&p).unwrap()).unwrap()
    });

    let client = ManyClient::new(server, server_id, key).unwrap();
    let http = tiny_http::Server::http(addr).unwrap();

    // TODO: parallelize this.
    for request in http.incoming_requests() {
        let path = request.url();
        match request.method() {
            Method::Get => {
                let result = client.call_(
                    "kvstore.get",
                    GetArgs {
                        key: format!("http/{}", path).into_bytes().into(),
                    },
                );
                match result {
                    Ok(result) => {
                        let GetReturns { value } = minicbor::decode(&result).unwrap();
                        match value {
                            None => request.respond(Response::empty(404)).unwrap(),
                            Some(value) => {
                                let mimetype = new_mime_guess::from_path(path).first();
                                let response = Response::empty(200)
                                    .with_data(value.as_slice(), Some(value.len()));
                                let response = if let Some(mimetype) = mimetype {
                                    response.with_header(
                                        Header::from_bytes("Content-Type", mimetype.essence_str())
                                            .unwrap(),
                                    )
                                } else {
                                    response
                                };

                                // Ignore errors on return.
                                let _ = request.respond(response);
                            }
                        }
                    }
                    Err(_) => request.respond(Response::empty(500)).unwrap(),
                }
            }
            // Method::Head => {}
            // Method::Options => {}
            x => {
                warn!("Received unknown method: {}", x);
                let _ = request.respond(Response::empty(StatusCode::from(405)));
            }
        }
    }
}
