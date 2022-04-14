use clap::Parser;
use many::server::module::kvstore::{GetArgs, GetReturns, PutArgs, PutReturns};
use many::server::module::r#async;
use many::types::identity::cose::CoseKeyIdentity;
use many::{Identity, ManyError};
use many_client::ManyClient;
use std::io::Read;
use std::path::PathBuf;
use tracing_subscriber::filter::LevelFilter;

#[derive(Parser)]
struct Opts {
    /// Many server URL to connect to.
    #[clap(default_value = "http://localhost:8000")]
    server: String,

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

    #[clap(subcommand)]
    subcommand: SubCommand,
}

#[derive(Parser)]
enum SubCommand {
    /// Get a value from the key-value store.
    Get(GetOpt),

    /// Put a value in the store.
    Put(PutOpt),
}

#[derive(Parser)]
struct GetOpt {
    /// The key to get.
    key: String,

    /// If the key is passed as an hexadecimal string, pass this key.
    #[clap(long)]
    hex_key: bool,

    /// Whether to output using hexadecimal, or regular value.
    #[clap(long)]
    hex: bool,
}

#[derive(Parser)]
struct PutOpt {
    /// The key to set.
    key: String,

    /// If the key is a hexadecimal string, pass this flag.
    #[clap(long)]
    hex_key: bool,

    /// The value to set. Use `--stdin` to read the value from STDIN.
    #[clap(conflicts_with = "stdin")]
    value: Option<String>,

    /// Use this flag to use STDIN to get the value.
    #[clap(long, conflicts_with = "value")]
    stdin: bool,
}

fn get(client: ManyClient, key: &[u8], hex: bool) -> Result<(), ManyError> {
    let arguments = GetArgs {
        key: key.to_vec().into(),
    };

    let payload = client.call_("kvstore.get", arguments)?;
    if payload.is_empty() {
        Err(ManyError::unexpected_empty_response())
    } else {
        let result: GetReturns = minicbor::decode(&payload)
            .map_err(|e| ManyError::deserialization_error(e.to_string()))?;
        let value = result.value.unwrap();

        if hex {
            println!("{}", hex::encode(value.as_slice()));
        } else {
            std::io::Write::write_all(&mut std::io::stdout(), &value).unwrap();
        }
        Ok(())
    }
}

fn put(client: ManyClient, key: &[u8], value: Vec<u8>) -> Result<(), ManyError> {
    let arguments = PutArgs {
        key: key.to_vec().into(),
        value: value.into(),
    };

    let response = client.call("kvstore.put", arguments)?;
    let payload = &response.data?;
    if payload.is_empty() {
        if response
            .attributes
            .get::<r#async::attributes::AsyncAttribute>()
            .is_ok()
        {
            eprintln!("Async response received...");
            Ok(())
        } else {
            Err(ManyError::unexpected_empty_response())
        }
    } else {
        let _: PutReturns = minicbor::decode(payload)
            .map_err(|e| ManyError::deserialization_error(e.to_string()))?;
        Ok(())
    }
}

fn main() {
    let Opts {
        pem,
        server,
        server_id,
        subcommand,
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

    let client = ManyClient::new(&server, server_id, key).unwrap();
    let result = match subcommand {
        SubCommand::Get(GetOpt { key, hex_key, hex }) => {
            let key = if hex_key {
                hex::decode(&key).unwrap()
            } else {
                key.into_bytes()
            };
            get(client, &key, hex)
        }
        SubCommand::Put(PutOpt {
            key,
            hex_key,
            value,
            stdin,
        }) => {
            let key = if hex_key {
                hex::decode(&key).unwrap()
            } else {
                key.into_bytes()
            };
            let value = if stdin {
                let mut value = Vec::new();
                std::io::stdin().read_to_end(&mut value).unwrap();
                value
            } else {
                value.expect("Must pass a value").into_bytes()
            };
            put(client, &key, value)
        }
    };

    if let Err(err) = result {
        eprintln!(
            "Error returned by server:\n|  {}\n",
            err.to_string()
                .split('\n')
                .collect::<Vec<&str>>()
                .join("\n|  ")
        );
        std::process::exit(1);
    }
}
