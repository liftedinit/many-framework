use clap::{Args, Parser};
use itertools::Itertools;
use many_client::client::blocking::ManyClient;
use many_error::ManyError;
use many_identity::{Address, Identity};
use many_modules::ledger::extended_info::visual_logo::VisualTokenLogo;
use many_modules::ledger::extended_info::TokenExtendedInfo;
use many_modules::ledger::{
    TokenAddExtendedInfoArgs, TokenAddExtendedInfoReturns, TokenCreateArgs, TokenCreateReturns,
    TokenInfoArgs, TokenInfoReturns, TokenRemoveExtendedInfoArgs, TokenRemoveExtendedInfoReturns,
    TokenUpdateArgs, TokenUpdateReturns,
};
use many_types::ledger::{LedgerTokensAddressMap, TokenAmount, TokenInfoSummary, TokenMaybeOwner};
use many_types::{AttributeRelatedIndex, Memo};
use std::str::FromStr;

#[derive(Parser)]
pub struct CommandOpt {
    #[clap(subcommand)]
    /// Token subcommand to execute.
    subcommand: SubcommandOpt,
}

#[derive(Parser)]
enum SubcommandOpt {
    /// Create a new token
    Create(CreateTokenOpt),

    /// Update an existing token
    Update(UpdateTokenOpt),

    /// Add extended information to token
    AddExtInfo(AddExtInfoOpt),

    /// Remove extended information from token
    RemoveExtInfo(RemoveExtInfoOpt),

    /// Get token info
    Info(InfoOpt),
}

#[derive(Args)]
struct InfoOpt {
    symbol: Address,

    #[clap(long)]
    indices: Option<Vec<u32>>, // TODO: Use Index
}

#[derive(Args)]
struct InitialDistribution {
    #[clap(long)]
    id: Address,

    #[clap(long)]
    amount: u64,
}

#[derive(Parser)]
struct CreateTokenOpt {
    name: String,
    ticker: String,
    decimals: u64,

    #[clap(long)]
    owner: Option<TokenMaybeOwner>,

    #[clap(long, action = clap::ArgAction::Append, number_of_values = 2, value_names = &["IDENTITY", "AMOUNT"])]
    initial_distribution: Option<Vec<String>>,

    #[clap(long)]
    maximum_supply: Option<u64>,

    #[clap(long)]
    extended_info: Option<String>,
}

#[derive(Parser)]
struct UpdateTokenOpt {
    symbol: Address,

    #[clap(long)]
    name: Option<String>,

    #[clap(long)]
    ticker: Option<String>,

    #[clap(long)]
    decimals: Option<u64>,

    #[clap(long)]
    owner: Option<TokenMaybeOwner>,

    #[clap(long)]
    #[clap(parse(try_from_str = Memo::try_from))]
    memo: Option<Memo>,
}

#[derive(Parser)]
enum CreateExtInfoOpt {
    Memo(MemoOpt),
    Logo(LogoOpt),
}

#[derive(Parser)]
struct MemoOpt {
    #[clap(parse(try_from_str = Memo::try_from))]
    memo: Memo,
}

#[derive(Parser)]
struct LogoOpt {
    #[clap(subcommand)]
    logo_type: CreateLogoOpt,
}

#[derive(Parser)]
enum CreateLogoOpt {
    Unicode(UnicodeLogoOpt),
    Image(ImageLogoOpt),
}

#[derive(Parser)]
struct UnicodeLogoOpt {
    glyph: char,
}

#[derive(Parser)]
struct ImageLogoOpt {
    content_type: String,
    data: String,
}

#[derive(Parser)]
struct AddExtInfoOpt {
    symbol: Address,

    #[clap(subcommand)]
    ext_info_type: CreateExtInfoOpt,
}

#[derive(Parser)]
struct RemoveExtInfoOpt {
    symbol: Address,

    indices: Vec<u32>, // TODO: Use Index
}

fn create_token(client: ManyClient<impl Identity>, opts: CreateTokenOpt) -> Result<(), ManyError> {
    let initial_distribution = opts.initial_distribution.map(|initial_distribution| {
        initial_distribution
            .into_iter()
            .chunks(2)
            .into_iter()
            .map(|mut chunk| {
                (
                    Address::from_str(&chunk.next().unwrap()).unwrap(),
                    TokenAmount::from(chunk.next().unwrap().parse::<u64>().unwrap()),
                )
            })
            .collect::<LedgerTokensAddressMap>()
    });

    let extended_info: Option<TokenExtendedInfo> = if let Some(data) = opts.extended_info {
        Some(
            minicbor::decode(
                &cbor_diag::parse_diag(data)
                    .map_err(ManyError::unknown)?
                    .to_bytes(),
            )
            .map_err(ManyError::deserialization_error)?,
        )
    } else {
        None
    };

    let args = TokenCreateArgs {
        summary: TokenInfoSummary {
            name: opts.name,
            ticker: opts.ticker,
            decimals: opts.decimals,
        },
        owner: opts.owner,
        initial_distribution,
        maximum_supply: opts.maximum_supply.map(TokenAmount::from),
        extended_info,
    };
    let response = client.call("tokens.create", args)?;
    let payload = crate::wait_response(client, response)?;
    let result: TokenCreateReturns =
        minicbor::decode(&payload).map_err(|e| ManyError::deserialization_error(e.to_string()))?;

    println!("{result:#?}");
    Ok(())
}

fn update_token(client: ManyClient<impl Identity>, opts: UpdateTokenOpt) -> Result<(), ManyError> {
    let args = TokenUpdateArgs {
        symbol: opts.symbol,
        name: opts.name,
        ticker: opts.ticker,
        decimals: opts.decimals,
        owner: opts.owner,
        memo: opts.memo,
    };
    let response = client.call("tokens.update", args)?;
    let payload = crate::wait_response(client, response)?;
    let _result: TokenUpdateReturns =
        minicbor::decode(&payload).map_err(|e| ManyError::deserialization_error(e.to_string()))?;

    Ok(())
}

fn add_ext_info(client: ManyClient<impl Identity>, opts: AddExtInfoOpt) -> Result<(), ManyError> {
    let extended_info = match opts.ext_info_type {
        CreateExtInfoOpt::Memo(opts) => TokenExtendedInfo::new().with_memo(opts.memo).unwrap(),
        CreateExtInfoOpt::Logo(opts) => {
            let mut logo = VisualTokenLogo::new();
            match opts.logo_type {
                CreateLogoOpt::Unicode(opts) => {
                    logo.unicode_front(opts.glyph);
                }
                CreateLogoOpt::Image(opts) => {
                    logo.image_front(opts.content_type, opts.data.into_bytes())
                }
            }
            TokenExtendedInfo::new().with_visual_logo(logo).unwrap()
        }
    };

    let args = TokenAddExtendedInfoArgs {
        symbol: opts.symbol,
        extended_info,
    };
    let response = client.call("tokens.addExtendedInfo", args)?;
    let payload = crate::wait_response(client, response)?;
    let _result: TokenAddExtendedInfoReturns =
        minicbor::decode(&payload).map_err(|e| ManyError::deserialization_error(e.to_string()))?;
    Ok(())
}

fn remove_ext_info(
    client: ManyClient<impl Identity>,
    opts: RemoveExtInfoOpt,
) -> Result<(), ManyError> {
    let args = TokenRemoveExtendedInfoArgs {
        symbol: opts.symbol,
        extended_info: opts
            .indices
            .into_iter()
            .map(AttributeRelatedIndex::new)
            .collect(),
    };
    let response = client.call("tokens.removeExtendedInfo", args)?;
    let payload = crate::wait_response(client, response)?;
    let _result: TokenRemoveExtendedInfoReturns =
        minicbor::decode(&payload).map_err(|e| ManyError::deserialization_error(e.to_string()))?;
    Ok(())
}

fn info_token(client: ManyClient<impl Identity>, opts: InfoOpt) -> Result<(), ManyError> {
    let args = TokenInfoArgs {
        symbol: opts.symbol,
        extended_info: opts
            .indices
            .map(|v| v.into_iter().map(AttributeRelatedIndex::new).collect_vec()),
    };
    let response = client.call("tokens.info", args)?;
    let payload = crate::wait_response(client, response)?;
    let result: TokenInfoReturns =
        minicbor::decode(&payload).map_err(|e| ManyError::deserialization_error(e.to_string()))?;

    println!("{result:#?}");
    Ok(())
}

pub fn tokens(client: ManyClient<impl Identity>, opts: CommandOpt) -> Result<(), ManyError> {
    match opts.subcommand {
        SubcommandOpt::Create(opts) => create_token(client, opts),
        SubcommandOpt::Update(opts) => update_token(client, opts),
        SubcommandOpt::AddExtInfo(opts) => add_ext_info(client, opts),
        SubcommandOpt::RemoveExtInfo(opts) => remove_ext_info(client, opts),
        SubcommandOpt::Info(opts) => info_token(client, opts),
    }
}
