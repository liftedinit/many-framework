use clap::Parser;
use minicbor::data::Tag;
use minicbor::encode::{Error, Write};
use minicbor::{Decoder, Encoder};
use omni::identity::cose::CoseKeyIdentity;
use omni::{Identity, OmniClient, OmniError};
use omni_kvstore::module::get::{GetArgs, GetReturns};
use omni_kvstore::module::put::{PutArgs, PutReturns};
use std::fmt::{Display, Formatter};
use std::io::Read;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use tiny_http::{Header, Method, Response, StatusCode};
use tracing::warn;
use tracing_subscriber::filter::LevelFilter;

#[derive(Parser)]
struct Opts {
    /// Omni server URL to connect to. It must implement a KV-Store attribute.
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
    tracing_subscriber::fmt().with_max_level(log_level).init();

    let server_id = server_id.unwrap_or_default();
    let key = pem.map_or_else(CoseKeyIdentity::anonymous, |p| {
        CoseKeyIdentity::from_pem(&std::fs::read_to_string(&p).unwrap()).unwrap()
    });

    let client = OmniClient::new(server, server_id, key).unwrap();
    let http = tiny_http::Server::http(addr).unwrap();

    // TODO: parallelize this.
    for request in http.incoming_requests() {
        let path = request.url();
        match request.method() {
            Method::Get => {
                let result = client.call_(
                    "kvstore.get",
                    GetArgs {
                        key: format!("http/{}", path).into_bytes(),
                        proof: None,
                    },
                );
                match result {
                    Ok(result) => {
                        let GetReturns { value, proof, hash } = minicbor::decode(&result).unwrap();
                        let value = value.unwrap();
                        let mimetype = new_mime_guess::from_path(path).first();
                        let response =
                            Response::empty(200).with_data(value.as_slice(), Some(value.len()));
                        let response = if let Some(mimetype) = mimetype {
                            response.with_header(
                                Header::from_bytes("Content-Type", mimetype.essence_str()).unwrap(),
                            )
                        } else {
                            response
                        };

                        // Ignore errors on return.
                        let _ = request.respond(response);
                    }
                    Err(_) => request.respond(Response::empty(404)).unwrap(),
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
