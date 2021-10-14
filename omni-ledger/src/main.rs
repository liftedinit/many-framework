use clap::Clap;
use omni::cbor::message::RequestMessageBuilder;
use omni::Identity;
use ring::signature::KeyPair;
use std::net::IpAddr;
use std::path::PathBuf;
use tracing_subscriber::filter::LevelFilter;

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
    // Pem file to use for the key. If omitted, the anonymous identity will be used.
    #[clap(long)]
    pem: Option<PathBuf>,

    /// Increase output logging verbosity to DEBUG level.
    #[clap(short, long)]
    verbose: bool,

    /// Suppress all output logging (overrides --verbose).
    #[clap(short, long)]
    quiet: bool,

    #[clap(long)]
    server: IpAddr,

    #[clap(subcommand)]
    subcommand: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    Balance(BalanceOpt),
    Mint(MintOpt),
    Send(SendOpt),
}

#[derive(Clap)]
struct BalanceOpt {
    account: Option<Identity>,
}

#[derive(Clap)]
struct MintOpt {
    /// Account to mint the tokens into.
    account: Identity,

    /// Amount of tokens to mint.
    amount: u128,
}

#[derive(Clap)]
struct SendOpt {
    /// The account to send to.
    to: Identity,

    /// Amount of tokens to send.
    amount: u128,
}

fn main() {
    let opt: Opt = Opt::parse();
    let log_level = if opt.quiet {
        LevelFilter::OFF
    } else if opt.verbose {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };
    tracing_subscriber::fmt().with_max_level(log_level).init();

    let (from_identity, keypair) = opt.pem.map_or_else(
        || (Identity::anonymous(), None),
        |pem| {
            let bytes = std::fs::read(pem).unwrap();
            let content = pem::parse(bytes).unwrap();

            let keypair =
                ring::signature::Ed25519KeyPair::from_pkcs8_maybe_unchecked(&content.contents)
                    .unwrap();

            (
                Identity::public_key(to_der(keypair.public_key().as_ref().to_vec())),
                Some(keypair),
            )
        },
    );

    match opt.subcommand {
        SubCommand::Balance(o) => {
            let message = RequestMessageBuilder::default()
                .version(1)
                .from(from_identity)
                .method("balance".to_string())
                .build()
                .unwrap();

            let bytes = message.to_cose(keypair.as_ref()).encode().unwrap();
            println!("{}", hex::encode(&bytes));
        }
        SubCommand::Mint(o) => {
            let mut data = Vec::new();
            let mut e = minicbor::Encoder::new(&mut data);
            e.array(3).unwrap();
            e.encode(o.account).unwrap();
            e.encode((o.amount >> 64) as u64).unwrap();
            e.encode(o.amount as u64).unwrap();

            let message = RequestMessageBuilder::default()
                .version(1)
                .from(from_identity)
                .method("mint".to_string())
                .data(data)
                .build()
                .unwrap();

            let bytes = message.to_cose(keypair.as_ref()).encode().unwrap();
            println!("{}", base64::encode(&bytes));
        }
        SubCommand::Send(o) => {
            let mut data = Vec::new();
            let mut e = minicbor::Encoder::new(&mut data);
            e.array(3).unwrap();
            e.encode(o.to).unwrap();
            e.encode((o.amount >> 64) as u64).unwrap();
            e.encode(o.amount as u64).unwrap();

            let message = RequestMessageBuilder::default()
                .version(1)
                .from(from_identity)
                .method("send".to_string())
                .data(data)
                .build()
                .unwrap();

            let bytes = message.to_cose(keypair.as_ref()).encode().unwrap();
            println!("{}", base64::encode(&bytes));
        }
    }
}
