use clap::Parser;
use minicbor::data::Tag;
use minicbor::encode::{Error, Write};
use minicbor::{Decoder, Encoder};
use num_bigint::BigUint;
use omni::{Identity, OmniClient, OmniError};
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

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
    symbol: Option<String>,
}

#[derive(Parser)]
struct TargetCommandOpt {
    /// The account or target identity.
    identity: Identity,

    /// The amount of token to mint.
    amount: BigUint,

    /// The symbol to check the balance of.
    symbol: Option<String>,
}

fn balance(
    client: OmniClient,
    identity: Option<Identity>,
    symbol: Option<String>,
) -> Result<(), OmniError> {
    let payload = client.call_("ledger.balance", (identity, symbol))?;

    if payload.is_empty() {
        Err(OmniError::unexpected_empty_response())
    } else {
        let balance: Amount = minicbor::decode(&payload).unwrap();
        println!("{}", balance);

        Ok(())
    }
}

fn mint(
    client: OmniClient,
    destination: Identity,
    amount: BigUint,
    symbol: Option<String>,
) -> Result<(), OmniError> {
    let payload = client.call_("ledger.mint", (destination, Amount(amount), symbol))?;
    if payload.is_empty() {
        Err(OmniError::unexpected_empty_response())
    } else {
        minicbor::display(&payload);
        Ok(())
    }
}

fn burn(
    client: OmniClient,
    destination: Identity,
    amount: BigUint,
    symbol: Option<String>,
) -> Result<(), OmniError> {
    let payload = client.call_("ledger.burn", (destination, Amount(amount), symbol))?;
    if payload.is_empty() {
        Err(OmniError::unexpected_empty_response())
    } else {
        minicbor::display(&payload);
        Ok(())
    }
}

fn send(
    client: OmniClient,
    destination: Identity,
    amount: BigUint,
    symbol: Option<String>,
) -> Result<(), OmniError> {
    if client.id.is_anonymous() {
        Err(OmniError::invalid_identity())
    } else {
        let payload = client.call_("ledger.send", (destination, Amount(amount), symbol))?;
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
    } = Opts::parse();

    let server_id = server_id.unwrap_or_default();

    // If `pem` is not provided, use anonymous and don't sign.
    let (from_identity, keypair) = pem.map_or_else(
        || (Identity::anonymous(), None),
        |pem| {
            let bytes = std::fs::read(pem).unwrap();
            let (id, keypair) = Identity::from_pem_public(bytes).unwrap();
            (id, Some(keypair))
        },
    );

    let client = OmniClient::new(&server, server_id, from_identity, keypair).unwrap();
    let result = match subcommand {
        SubCommand::Balance(BalanceOpt { identity, symbol }) => {
            let identity = identity.map(|ref identity| {
                Identity::from_str(identity)
                    .or_else(|_| {
                        let bytes = std::fs::read(PathBuf::from(identity))?;

                        Ok(Identity::from_pem_public(bytes).unwrap().0)
                    })
                    .map_err(|_: std::io::Error| ())
                    .unwrap()
            });

            balance(client, identity, symbol)
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
