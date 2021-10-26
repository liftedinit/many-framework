pub mod identity;
pub mod message;

use clap::Parser as Clap;
use minicose::{CoseKey, CoseSign1, Ed25519CoseKey, Ed25519CoseKeyBuilder};
use omni::message::{encode_cose_sign1_from_request, RequestMessage, RequestMessageBuilder};
use omni::Identity;
use ring::signature::KeyPair;
use std::convert::TryFrom;
use std::path::PathBuf;

#[derive(Clap)]
struct Opt {
    #[clap(subcommand)]
    subcommand: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    /// Transform a textual ID into its binary value, or the other way around.
    Id(IdOpt),

    /// Shows the identity ID from a PEM file.
    IdOf(IdOfOpt),

    /// Creates a message and output it.
    Message(MessageOpt),
}

#[derive(Clap)]
struct IdOpt {
    /// An hexadecimal value to encode, or an identity textual format to decode.
    arg: String,
}

#[derive(Clap)]
struct IdOfOpt {
    /// The pem file to read from.
    pem: PathBuf,

    /// Whether or not this public key is addressable (e.g. a Network).
    #[clap(long)]
    addressable: bool,

    /// Whether to display the key in hexadecimal.
    #[clap(long)]
    hex: bool,
}

#[derive(Clap)]
struct MessageOpt {
    /// A pem file to sign the message. If this is omitted, the message will be anonymous.
    #[clap(long)]
    pem: Option<PathBuf>,

    /// Timestamp.
    #[clap(long)]
    timestamp: Option<String>,

    /// If true, prints out the hex value of the message bytes.
    #[clap(long, conflicts_with("base64"))]
    hex: bool,

    /// If true, prints out the base64 value of the message bytes.
    #[clap(long, conflicts_with("hex"))]
    base64: bool,

    /// The server to connect to.
    #[clap(long)]
    server: Option<String>,

    /// The identity to send it to.
    to: String,

    /// The method to call.
    method: String,

    /// The content of the message itself (its payload).
    data: Option<String>,
}

fn main() {
    let opt: Opt = Opt::parse();

    match opt.subcommand {
        SubCommand::Id(o) => {
            if let Ok(data) = hex::decode(&o.arg) {
                if let Ok(i) = Identity::try_from(data.as_slice()) {
                    println!("{}", i);
                } else {
                    eprintln!("Invalid hexadecimal.");
                    std::process::exit(1);
                }
            } else {
                let i = Identity::try_from(o.arg.to_string()).unwrap();
                println!("{}", hex::encode(&i.to_vec()));
            }
        }
        SubCommand::IdOf(o) => {
            let bytes = std::fs::read(o.pem).unwrap();
            let content = pem::parse(bytes).unwrap();

            let keypair =
                ring::signature::Ed25519KeyPair::from_pkcs8_maybe_unchecked(&content.contents)
                    .unwrap();

            let x = keypair.public_key().as_ref().to_vec();
            let cose_key: CoseKey = Ed25519CoseKeyBuilder::default()
                .x(x)
                .build()
                .unwrap()
                .into();

            // Create the identity from the public key hash.
            let id = if o.addressable {
                Identity::addressable(&cose_key)
            } else {
                Identity::public_key(&cose_key)
            };

            if o.hex {
                println!("{}", hex::encode(id.to_vec()));
            } else {
                println!("{}", id);
            }
        }
        SubCommand::Message(o) => {
            // If `pem` is not provided, use anonymous and don't sign.
            let (from_identity, keypair) = o.pem.map_or_else(
                || (Identity::anonymous(), None),
                |pem| {
                    let bytes = std::fs::read(pem).unwrap();
                    let content = pem::parse(bytes).unwrap();

                    let keypair = ring::signature::Ed25519KeyPair::from_pkcs8_maybe_unchecked(
                        &content.contents,
                    )
                    .unwrap();

                    let x = keypair.public_key().as_ref().to_vec();
                    let cose_key: Ed25519CoseKey = Ed25519CoseKeyBuilder::default()
                        .x(x)
                        .build()
                        .unwrap()
                        .into();

                    (
                        Identity::public_key(&cose_key.to_public_key().unwrap().into()),
                        Some(keypair),
                    )
                },
            );
            let to_identity = Identity::try_from(o.to).unwrap();

            let data = o
                .data
                .map_or(vec![], |d| cbor_diag::parse_diag(&d).unwrap().to_bytes());
            let message: RequestMessage = RequestMessageBuilder::default()
                .version(1)
                .from(from_identity)
                .to(to_identity)
                .method(o.method)
                .data(data)
                .build()
                .unwrap();

            let cose = encode_cose_sign1_from_request(message, from_identity, &keypair).unwrap();
            let bytes = cose.to_bytes().unwrap();

            if o.hex {
                println!("{}", hex::encode(&bytes));
            } else if o.base64 {
                println!("{}", base64::encode(&bytes));
            } else if let Some(s) = o.server {
                let client = reqwest::blocking::Client::new();
                let response = client.post(s).body(bytes).send().unwrap();

                let body = response.bytes().unwrap();
                let bytes = body.to_vec();
                let cose_sign1 = CoseSign1::from_bytes(&bytes).unwrap();
                let response = message::decode_response_from_cose_sign1(cose_sign1, None).unwrap();

                match response.data {
                    Some(Ok(payload)) => {
                        println!("{}", cbor_diag::parse_bytes(&payload).unwrap().to_diag());
                        std::process::exit(0);
                    }
                    None => {
                        std::process::exit(0);
                    }
                    Some(Err(err)) => {
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
            } else {
                panic!("Must specify one of hex, base64 or server...");
            }
        }
    }
}
