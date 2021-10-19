mod cbor;
mod identity;

use cbor::cose::CoseSign1;
use cbor::message::RequestMessageBuilder;
use cbor::value::CborValue;
use clap::Clap;
use identity::Identity;
use minicbor::Encoder;
use omni::cbor::message::ResponseMessage;
use ring::signature::KeyPair;
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::path::PathBuf;

fn to_der(key: Vec<u8>) -> Vec<u8> {
    use simple_asn1::{
        oid, to_der,
        ASN1Block::{BitString, ObjectIdentifier, Sequence},
    };

    let public_key = key;
    let id_ed25519 = oid!(1, 3, 101, 112);
    let algorithm = Sequence(0, vec![ObjectIdentifier(0, id_ed25519)]);
    let subject_public_key = BitString(0, public_key.len() * 8, public_key);
    let subject_public_key_info = Sequence(0, vec![algorithm, subject_public_key]);
    to_der(&subject_public_key_info).unwrap()
}

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
            // Create the identity from the public key hash.
            let id = Identity::public_key(to_der(keypair.public_key().as_ref().to_vec()));
            println!("{}", id);
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

                    (
                        Identity::public_key(to_der(keypair.public_key().as_ref().to_vec())),
                        Some(keypair),
                    )
                },
            );
            let to_identity = Identity::try_from(o.to).unwrap();

            let data = o
                .data
                .map_or(vec![], |d| cbor_diag::parse_diag(&d).unwrap().to_bytes());
            let message = RequestMessageBuilder::default()
                .version(1)
                .from(from_identity)
                .to(to_identity)
                .method(o.method)
                .data(data)
                .build()
                .unwrap();

            let cose = message.to_cose(keypair.as_ref());
            let mut bytes = Vec::<u8>::new();
            minicbor::encode(cose, &mut bytes).unwrap();

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

                let response =
                    ResponseMessage::from_bytes(&cose_sign1.payload.unwrap_or_default()).unwrap();

                match response.data {
                    Some(Ok(payload)) => {
                        println!("{}", cbor_diag::parse_bytes(&payload).unwrap().to_diag());
                        std::process::exit(0);
                    }
                    None => {
                        std::process::exit(0);
                    }
                    Some(Err(err)) => {
                        eprintln!("An error happened:\n{}\n", err);
                        std::process::exit(1);
                    }
                }
            } else {
                panic!("Must specify one of hex, base64 or server...");
            }
        }
    }
}
