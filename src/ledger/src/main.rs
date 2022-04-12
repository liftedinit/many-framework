use clap::{ArgGroup, Parser};
use many::hsm::{HSMMechanismType, HSMSessionType, HSMUserType, HSM};
use many::message::ResponseMessage;
use many::server::module::ledger::{
    BalanceArgs, BalanceReturns, BurnArgs, InfoReturns, MintArgs, SendArgs,
};
use many::types::identity::cose::CoseKeyIdentity;
use many::types::ledger::{Symbol, TokenAmount};
use many::{Identity, ManyError};
use many_client::ManyClient;
use minicbor::data::Tag;
use minicbor::encode::{Error, Write};
use minicbor::{Decoder, Encoder};
use num_bigint::BigUint;
use tracing::{error, info, trace};
use tracing_subscriber::filter::LevelFilter;
use many::server::module::r#async::attributes::AsyncAttribute;

use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::str::FromStr;

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
#[clap(
    group(
        ArgGroup::new("hsm")
        .multiple(true)
        .args(&["module", "slot", "keyid"])
        .requires_all(&["module", "slot", "keyid"])
    )
)]
struct Opts {
    /// Many server URL to connect to.
    #[clap(default_value = "http://localhost:8000")]
    server: String,

    /// The identity of the server (an identity string), or anonymous if you don't know it.
    server_id: Option<Identity>,

    /// A PEM file for the identity. If not specified, anonymous will be used.
    #[clap(long)]
    pem: Option<PathBuf>,

    /// HSM PKCS#11 module path
    #[clap(long, conflicts_with("pem"))]
    module: Option<PathBuf>,

    /// HSM PKCS#11 slot ID
    #[clap(long, conflicts_with("pem"))]
    slot: Option<u64>,

    /// HSM PKCS#11 key ID
    #[clap(long, conflicts_with("pem"))]
    keyid: Option<String>,

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
    /// identity) or an identity string. If omitted it will use the identity of the caller.
    identity: Option<String>,

    /// The symbol to check the balance of. This can either be an identity or
    /// a local name for a symbol. If it doesn't parse to an identity an
    /// additional call will be made to retrieve local names.
    #[clap(last = true)]
    symbols: Vec<String>,
}

#[derive(Parser)]
struct TargetCommandOpt {
    /// The account or target identity.
    identity: Identity,

    /// The amount of token to mint.
    amount: BigUint,

    /// The symbol to use.  This can either be an identity or
    /// a local name for a symbol. If it doesn't parse to an identity an
    /// additional call will be made to retrieve local names.
    symbol: String,
}

fn resolve_symbol(client: &ManyClient, symbol: String) -> Result<Identity, ManyError> {
    if let Ok(symbol) = Identity::from_str(&symbol) {
        Ok(symbol)
    } else {
        // Get info.
        let info: InfoReturns = minicbor::decode(&client.call_("ledger.info", ())?).unwrap();
        info.local_names
            .into_iter()
            .find(|(_, y)| y == &symbol)
            .map(|(x, _)| x)
            .ok_or_else(|| ManyError::unknown(format!("Could not resolve symbol '{}'", &symbol)))
    }
}

fn balance(
    client: ManyClient,
    account: Option<Identity>,
    symbols: Vec<String>,
) -> Result<(), ManyError> {
    // Get info.
    let info: InfoReturns = minicbor::decode(&client.call_("ledger.info", ())?).unwrap();
    let local_names: BTreeMap<String, Symbol> = info
        .local_names
        .iter()
        .map(|(x, y)| (y.clone(), *x))
        .collect();

    let argument = BalanceArgs {
        account,
        symbols: if symbols.is_empty() {
            None
        } else {
            Some(
                symbols
                    .iter()
                    .map(|x| {
                        if let Ok(i) = Identity::from_str(x) {
                            Ok(i)
                        } else if let Some(i) = local_names.get(x.as_str()) {
                            Ok(*i)
                        } else {
                            Err(ManyError::unknown(format!(
                                "Could not resolve symbol '{}'",
                                x
                            )))
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into(),
            )
        },
    };
    let payload = client.call_("ledger.balance", argument)?;

    if payload.is_empty() {
        Err(ManyError::unexpected_empty_response())
    } else {
        let balance: BalanceReturns = minicbor::decode(&payload).unwrap();
        for (symbol, amount) in balance.balances {
            if let Some(symbol_name) = info.local_names.get(&symbol) {
                println!("{:>12} {} ({})", amount, symbol_name, symbol);
            } else {
                println!("{:>12} {}", amount, symbol);
            }
        }

        Ok(())
    }
}

fn mint(
    client: ManyClient,
    account: Identity,
    amount: BigUint,
    symbol: String,
) -> Result<(), ManyError> {
    let symbol = resolve_symbol(&client, symbol)?;

    let arguments = MintArgs {
        account,
        symbol,
        amount: TokenAmount::from(amount),
    };
    let payload = client.call_("ledger.mint", arguments)?;
    if payload.is_empty() {
        Err(ManyError::unexpected_empty_response())
    } else {
        minicbor::display(&payload);
        Ok(())
    }
}

fn burn(
    client: ManyClient,
    account: Identity,
    amount: BigUint,
    symbol: String,
) -> Result<(), ManyError> {
    let symbol = resolve_symbol(&client, symbol)?;

    let arguments = BurnArgs {
        account,
        symbol,
        amount: TokenAmount::from(amount),
    };
    let payload = client.call_("ledger.burn", arguments)?;
    if payload.is_empty() {
        Err(ManyError::unexpected_empty_response())
    } else {
        minicbor::display(&payload);
        Ok(())
    }
}

fn send(
    client: ManyClient,
    to: Identity,
    amount: BigUint,
    symbol: String,
) -> Result<(), ManyError> {
    let symbol = resolve_symbol(&client, symbol)?;

    if client.id.identity.is_anonymous() {
        Err(ManyError::invalid_identity())
    } else {
        let arguments = SendArgs {
            from: None,
            to,
            symbol,
            amount: TokenAmount::from(amount),
        };
        let ResponseMessage {
            data, attributes, ..
        } = client.call("ledger.send", arguments)?;

        let payload = data?;
        if payload.is_empty() {
            let attr = attributes.get::<AsyncAttribute>()?;
            info!("Async token: {}", hex::encode(&attr.token));
            Ok(())
        } else {
            minicbor::display(&payload);
            Ok(())
        }
    }
}

fn main() {
    let Opts {
        pem,
        module,
        slot,
        keyid,
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
    let key = if let (Some(module), Some(slot), Some(keyid)) = (module, slot, keyid) {
        trace!("Getting user PIN");
        let pin = rpassword::prompt_password("Please enter the HSM user PIN: ")
            .expect("I/O error when reading HSM PIN");
        let keyid = hex::decode(keyid).expect("Failed to decode keyid to hex");

        {
            let mut hsm = HSM::get_instance().expect("HSM mutex poisoned");
            hsm.init(module, keyid)
                .expect("Failed to initialize HSM module");

            // The session will stay open until the application terminates
            hsm.open_session(slot, HSMSessionType::RO, Some(HSMUserType::User), Some(pin))
                .expect("Failed to open HSM session");
        }

        trace!("Creating CoseKeyIdentity");
        // Only ECDSA is supported at the moment. It should be easy to add support for new EC mechanisms
        CoseKeyIdentity::from_hsm(HSMMechanismType::ECDSA)
            .expect("Unable to create CoseKeyIdentity from HSM")
    } else {
        pem.map_or_else(CoseKeyIdentity::anonymous, |p| {
            CoseKeyIdentity::from_pem(&std::fs::read_to_string(&p).unwrap()).unwrap()
        })
    };

    let client = ManyClient::new(&server, server_id, key).unwrap();
    let result = match subcommand {
        SubCommand::Balance(BalanceOpt { identity, symbols }) => {
            let identity = identity.map(|identity| {
                Identity::from_str(&identity)
                    .or_else(|_| {
                        let bytes = std::fs::read_to_string(PathBuf::from(identity))?;

                        Ok(CoseKeyIdentity::from_pem(&bytes).unwrap().identity)
                    })
                    .map_err(|_: std::io::Error| ())
                    .expect("Unable to decode identity command-line argument")
            });

            balance(client, identity, symbols)
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
        error!(
            "Error returned by server:\n|  {}\n",
            err.to_string()
                .split('\n')
                .collect::<Vec<&str>>()
                .join("\n|  ")
        );
        std::process::exit(1);
    }
}
