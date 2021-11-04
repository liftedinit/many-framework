use clap::Parser as Clap;
use omni::message::RequestMessageBuilder;
use omni::Identity;
use ring::signature::KeyPair;
use std::net::IpAddr;
use std::path::PathBuf;
use tracing_subscriber::filter::LevelFilter;

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
    server: String,
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
            Identity::from_pem_public(bytes)
                .map(|(i, kp)| (i, Some(kp)))
                .unwrap()
        },
    );

    match opt.subcommand {
        SubCommand::Balance(o) => {
            let response = omni::message::send_raw(
                o.server,
                keypair.map(|kp| (from_identity, kp)),
                Identity::anonymous(),
                "ledger.balance",
                Vec::<u8>::new(),
            )
            .unwrap();

            let (high, low): (u64, u64) = minicbor::decode(response.as_slice()).unwrap();
            println!("{:?}", (high as u128) << 64 + (low as u128));
        }
        SubCommand::Mint(o) => {
            // let mut data = Vec::new();
            // let mut e = minicbor::Encoder::new(&mut data);
            // e.array(3).unwrap();
            // e.encode(o.account).unwrap();
            // e.encode((o.amount >> 64) as u64).unwrap();
            // e.encode(o.amount as u64).unwrap();
            //
            // let message = RequestMessageBuilder::default()
            //     .version(1)
            //     .from(from_identity)
            //     .method("mint".to_string())
            //     .data(data)
            //     .build()
            //     .unwrap();
            //
            // let bytes = message.to_cose(keypair.as_ref()).encode().unwrap();
            // println!("{}", base64::encode(&bytes));
            unreachable!()
        }
        SubCommand::Send(o) => {
            // let mut data = Vec::new();
            // let mut e = minicbor::Encoder::new(&mut data);
            // e.array(3).unwrap();
            // e.encode(o.to).unwrap();
            // e.encode((o.amount >> 64) as u64).unwrap();
            // e.encode(o.amount as u64).unwrap();
            //
            // let message = RequestMessageBuilder::default()
            //     .version(1)
            //     .from(from_identity)
            //     .method("send".to_string())
            //     .data(data)
            //     .build()
            //     .unwrap();
            //
            // let bytes = message.to_cose(keypair.as_ref()).encode().unwrap();
            // println!("{}", base64::encode(&bytes));
            unreachable!()
        }
    }
}
