use clap::Parser;
use many_client::client::blocking::ManyClient;
use many_error::ManyError;
use many_identity::{Address, Identity};
use many_modules::ledger::extended_info::TokenExtendedInfo;
use many_modules::ledger::{
    TokenAddExtendedInfoArgs, TokenAddExtendedInfoReturns, TokenCreateArgs, TokenCreateReturns,
    TokenRemoveExtendedInfoArgs, TokenRemoveExtendedInfoReturns, TokenUpdateArgs,
    TokenUpdateReturns,
};
use many_types::ledger::{LedgerTokensAddressMap, TokenAmount, TokenInfoSummary};
use many_types::{AttributeRelatedIndex, Memo};

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
}

#[derive(Parser)]
struct CreateTokenOpt {
    name: String,
    ticker: String,
    decimals: u64,

    #[clap(long)]
    owner: Option<Address>,

    #[clap(long)]
    initial_distribution: Option<String>,

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
    owner: Option<Address>,

    #[clap(long)]
    memo: Option<String>,
}

#[derive(Parser)]
struct AddExtInfoOpt {
    symbol: Address,

    extended_info: String,
}

#[derive(Parser)]
struct RemoveExtInfoOpt {
    symbol: Address,

    indices: Vec<u32>,
}

fn create_token(client: ManyClient<impl Identity>, opts: CreateTokenOpt) -> Result<(), ManyError> {
    let initial_distribution: Option<LedgerTokensAddressMap> =
        if let Some(data) = opts.initial_distribution {
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
        owner: opts.owner.map_or(None, |addr| Some(Some(addr))),
        initial_distribution,
        maximum_supply: opts.maximum_supply.map(|amount| TokenAmount::from(amount)),
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
        owner: opts.owner.map_or(None, |addr| Some(Some(addr))),
        memo: opts
            .memo
            .map(|memo| Memo::try_from(memo).expect("Unable to create Memo from String")),
    };
    let response = client.call("tokens.update", args)?;
    let payload = crate::wait_response(client, response)?;
    let _result: TokenUpdateReturns =
        minicbor::decode(&payload).map_err(|e| ManyError::deserialization_error(e.to_string()))?;

    Ok(())
}

fn add_ext_info(client: ManyClient<impl Identity>, opts: AddExtInfoOpt) -> Result<(), ManyError> {
    let extended_info: TokenExtendedInfo = minicbor::decode(
        &cbor_diag::parse_diag(opts.extended_info)
            .map_err(ManyError::unknown)?
            .to_bytes(),
    )
    .map_err(ManyError::deserialization_error)?;

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
            .map(|i| AttributeRelatedIndex::new(i))
            .collect(),
    };
    let response = client.call("tokens.removeExtendedInfo", args)?;
    let payload = crate::wait_response(client, response)?;
    let _result: TokenRemoveExtendedInfoReturns =
        minicbor::decode(&payload).map_err(|e| ManyError::deserialization_error(e.to_string()))?;
    Ok(())
}

pub fn tokens(client: ManyClient<impl Identity>, opts: CommandOpt) -> Result<(), ManyError> {
    match opts.subcommand {
        SubcommandOpt::Create(opts) => create_token(client, opts),
        SubcommandOpt::Update(opts) => update_token(client, opts),
        SubcommandOpt::AddExtInfo(opts) => add_ext_info(client, opts),
        SubcommandOpt::RemoveExtInfo(opts) => remove_ext_info(client, opts),
    }
}
