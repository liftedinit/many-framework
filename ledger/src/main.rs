use clap::Parser;
use minicbor::data::Tag;
use minicbor::encode::{Error, Write};
use minicbor::{Decoder, Encoder};
use num_bigint::BigUint;
use omni::identity::cose::CoseKeyIdentity;
use omni::{Identity, OmniClient, OmniError};
use omni_ledger::module::balance::{BalanceArgs, BalanceReturns};
use omni_ledger::module::burn::BurnArgs;
use omni_ledger::module::info::InfoReturns;
use omni_ledger::module::mint::MintArgs;
use omni_ledger::module::send::SendArgs;
use omni_ledger::utils::TokenAmount;
use omni_ledger::verify_proof;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use tracing_subscriber::filter::LevelFilter;

#[derive(Clone, Debug)]
#[repr(transparent)]
struct Amount(pub BigUint);

impl Display for Amount {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl minicbor::Encode for Amount {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.tag(Tag::PosBignum)?.bytes(&self.0.to_bytes_be())?;
        Ok(())
    }
}
impl<'b> minicbor::Decode<'b> for Amount {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        let t = d.tag()?;
        if t != Tag::PosBignum {
            Err(minicbor::decode::Error::Message("Invalid tag."))
        } else {
            Ok(Amount(BigUint::from_bytes_be(d.bytes()?)))
        }
    }
}

#[derive(Parser)]
struct Opts {
    /// Omni server URL to connect to.
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
    /// Read the balance of an account.
    Balance(BalanceOpt),

    /// Mint new tokens into an account.
    Mint(TargetCommandOpt),

    /// Burn tokens from an account.
    Burn(TargetCommandOpt),

    /// Send tokens to an account.
    Send(TargetCommandOpt),
}

#[derive(Parser)]
struct BalanceOpt {
    /// The identity to check. This can be a Pem file (which will be used to calculate a public
    /// identity) or an identity string. If omitted it will use the identity of the caller,
    /// and if there is none, it will ignore the call.
    identity: Option<String>,

    /// The symbol to check the balance of.
    symbols: Vec<String>,

    /// Whether to return a proof or not.
    #[clap(long)]
    proof: bool,
}

#[derive(Parser)]
struct TargetCommandOpt {
    /// The account or target identity.
    identity: Identity,

    /// The amount of token to mint.
    amount: BigUint,

    /// The symbol to check the balance of.
    symbol: String,
}

fn balance(
    client: OmniClient,
    account: Option<Identity>,
    symbols: Vec<String>,
    proof: bool,
) -> Result<(), OmniError> {
    let argument = BalanceArgs {
        account,
        symbols: if symbols.is_empty() {
            None
        } else {
            Some(symbols.clone().into())
        },
        proof: Some(proof),
    };
    let payload = client.call_("ledger.balance", argument)?;

    if payload.is_empty() {
        Err(OmniError::unexpected_empty_response())
    } else {
        let balance: BalanceReturns = minicbor::decode(&payload).unwrap();
        if let Some(balances) = balance.balances {
            for (symbol, amount) in balances {
                println!("{} {}", symbol, amount);
            }
        } else if let Some(p) = balance.proof {
            let info = client.call_("ledger.info", ())?;
            let info: InfoReturns = minicbor::decode(&info).unwrap();
            let balances = verify_proof(
                p.as_slice(),
                account.as_ref().unwrap_or_else(|| &client.id.identity),
                symbols.as_slice(),
                &info.hash.to_vec().try_into().unwrap(),
            )
            .unwrap();

            for (symbol, amount) in balances {
                println!("{} {}", symbol, amount);
            }
        } else {
            return Err(OmniError::unknown(
                "Server did not response with either a balance record or proof. This is an error"
                    .to_string(),
            ));
        }

        Ok(())
    }
}

fn mint(
    client: OmniClient,
    account: Identity,
    amount: BigUint,
    symbol: String,
) -> Result<(), OmniError> {
    let arguments = MintArgs {
        account,
        symbol: symbol.as_str(),
        amount: TokenAmount::from(amount),
    };
    let payload = client.call_("ledger.mint", arguments)?;
    if payload.is_empty() {
        Err(OmniError::unexpected_empty_response())
    } else {
        minicbor::display(&payload);
        Ok(())
    }
}

fn burn(
    client: OmniClient,
    account: Identity,
    amount: BigUint,
    symbol: String,
) -> Result<(), OmniError> {
    let arguments = BurnArgs {
        account,
        symbol: symbol.as_str(),
        amount: TokenAmount::from(amount),
    };
    let payload = client.call_("ledger.burn", arguments)?;
    if payload.is_empty() {
        Err(OmniError::unexpected_empty_response())
    } else {
        minicbor::display(&payload);
        Ok(())
    }
}

fn send(
    client: OmniClient,
    to: Identity,
    amount: BigUint,
    symbol: String,
) -> Result<(), OmniError> {
    if client.id.identity.is_anonymous() {
        Err(OmniError::invalid_identity())
    } else {
        let arguments = SendArgs {
            from: None,
            to,
            symbol: symbol.as_str(),
            amount: TokenAmount::from(amount),
        };
        let payload = client.call_("ledger.send", arguments)?;
        if payload.is_empty() {
            Err(OmniError::unexpected_empty_response())
        } else {
            minicbor::display(&payload);
            Ok(())
        }
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
    let key = pem.map_or_else(
        || CoseKeyIdentity::anonymous(),
        |p| CoseKeyIdentity::from_pem(&std::fs::read_to_string(&p).unwrap()).unwrap(),
    );

    let client = OmniClient::new(&server, server_id, key).unwrap();
    let result = match subcommand {
        SubCommand::Balance(BalanceOpt {
            identity,
            symbols,
            proof,
        }) => {
            let identity = identity.map(|ref identity| {
                Identity::from_str(identity)
                    .or_else(|_| {
                        let bytes = std::fs::read_to_string(PathBuf::from(identity))?;

                        Ok(CoseKeyIdentity::from_pem(&bytes).unwrap().identity)
                    })
                    .map_err(|_: std::io::Error| ())
                    .unwrap()
            });

            balance(client, identity, symbols, proof)
        }
        SubCommand::Mint(TargetCommandOpt {
            identity,
            amount,
            symbol,
        }) => mint(client, identity, amount, symbol),
        SubCommand::Burn(TargetCommandOpt {
            identity,
            amount,
            symbol,
        }) => burn(client, identity, amount, symbol),
        SubCommand::Send(TargetCommandOpt {
            identity,
            amount,
            symbol,
        }) => send(client, identity, amount, symbol),
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
