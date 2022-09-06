use crate::TargetCommandOpt;
use clap::Parser;
use many_client::ManyClient;
use many_error::ManyError;
use many_identity::Address;
use many_modules::account::features::multisig;
use many_modules::{events, ledger};
use many_protocol::ResponseMessage;
use many_types::ledger::TokenAmount;
use minicbor::bytes::ByteVec;
use tracing::info;

#[derive(Parser)]
pub struct CommandOpt {
    #[clap(subcommand)]
    /// Multisig subcommand to execute.
    subcommand: SubcommandOpt,
}

#[derive(Parser)]
struct SetDefaultsOpt {
    /// The account to set defaults of.
    target_account: Address,

    #[clap(flatten)]
    opts: MultisigArgOpt,
}

#[derive(Parser)]
enum SubcommandOpt {
    /// Submit a new transaction to be approved.
    Submit {
        /// The account to use as the source of the multisig command.
        account: Address,

        #[clap(flatten)]
        multisig_arg: MultisigArgOpt,

        #[clap(subcommand)]
        subcommand: SubmitOpt,
    },

    /// Approve a transaction.
    Approve(TransactionOpt),

    /// Revoke approval of a transaction.
    Revoke(TransactionOpt),

    /// Execute a transaction.
    Execute(TransactionOpt),

    /// Show the information of a multisig transaction.
    Info(TransactionOpt),

    /// Set new defaults for the multisig account.
    SetDefaults(SetDefaultsOpt),
}

#[derive(Parser)]
enum SubmitOpt {
    /// Send tokens to someone.
    Send(TargetCommandOpt),

    /// Set new defaults for the account.
    SetDefaults(SetDefaultsOpt),
}

fn parse_token(s: &str) -> Result<ByteVec, String> {
    hex::decode(s).map_err(|e| e.to_string()).map(|v| v.into())
}

#[derive(Parser)]
struct TransactionOpt {
    /// The transaction token, obtained when submitting a new transaction.
    #[clap(parse(try_from_str=parse_token))]
    token: ByteVec,
}

#[derive(Parser)]
struct MultisigArgOpt {
    /// The number of approvals needed to execute a transaction.
    #[clap(long)]
    threshold: Option<u64>,

    /// The timeout of a transaction.
    #[clap(long)]
    timeout: Option<humantime::Duration>,

    /// Whether to execute a transaction automatically when the threshold of
    /// approvals is reached.
    #[clap(long)]
    execute_automatically: Option<bool>,
}

fn submit_send(
    client: ManyClient,
    account: Address,
    multisig_arg: MultisigArgOpt,
    opts: TargetCommandOpt,
) -> Result<(), ManyError> {
    let TargetCommandOpt {
        account: from,
        identity,
        amount,
        symbol,
    } = opts;
    let MultisigArgOpt {
        threshold,
        timeout,
        execute_automatically,
    } = multisig_arg;
    let symbol = crate::resolve_symbol(&client, symbol)?;

    if client.id.identity.is_anonymous() {
        Err(ManyError::invalid_identity())
    } else {
        let transaction = events::AccountMultisigTransaction::Send(ledger::SendArgs {
            from: from.or(Some(account)),
            to: identity,
            symbol,
            amount: TokenAmount::from(amount),
        });
        let arguments = multisig::SubmitTransactionArgs {
            account,
            memo: None,
            transaction: Box::new(transaction),
            threshold,
            timeout_in_secs: timeout.map(|d| d.as_secs()),
            execute_automatically,
            data: None,
        };
        let response = client.call("account.multisigSubmitTransaction", arguments)?;

        let payload = crate::wait_response(client, response)?;
        let result: multisig::SubmitTransactionReturn = minicbor::decode(&payload)
            .map_err(|e| ManyError::deserialization_error(e.to_string()))?;

        info!(
            "Transaction Token: {}",
            hex::encode(result.token.as_slice())
        );
        Ok(())
    }
}

fn submit_set_defaults(
    client: ManyClient,
    account: Address,
    multisig_arg: MultisigArgOpt,
    target: Address,
    opts: MultisigArgOpt,
) -> Result<(), ManyError> {
    let MultisigArgOpt {
        threshold,
        timeout,
        execute_automatically,
    } = multisig_arg;

    if client.id.identity.is_anonymous() {
        Err(ManyError::invalid_identity())
    } else {
        let transaction = events::AccountMultisigTransaction::AccountMultisigSetDefaults(
            multisig::SetDefaultsArgs {
                account: target,
                threshold: opts.threshold,
                timeout_in_secs: opts.timeout.map(|d| d.as_secs()),
                execute_automatically: opts.execute_automatically,
            },
        );
        let arguments = multisig::SubmitTransactionArgs {
            account,
            memo: None,
            transaction: Box::new(transaction),
            threshold,
            timeout_in_secs: timeout.map(|d| d.as_secs()),
            execute_automatically,
            data: None,
        };
        let response = client.call("account.multisigSubmitTransaction", arguments)?;

        let payload = crate::wait_response(client, response)?;
        let result: multisig::SubmitTransactionReturn = minicbor::decode(&payload)
            .map_err(|e| ManyError::deserialization_error(e.to_string()))?;

        info!(
            "Transaction Token: {}",
            hex::encode(result.token.as_slice())
        );
        Ok(())
    }
}

fn submit(
    client: ManyClient,
    account: Address,
    multisig_arg: MultisigArgOpt,
    opts: SubmitOpt,
) -> Result<(), ManyError> {
    match opts {
        SubmitOpt::Send(target) => submit_send(client, account, multisig_arg, target),
        SubmitOpt::SetDefaults(SetDefaultsOpt {
            target_account,
            opts,
        }) => submit_set_defaults(client, account, multisig_arg, target_account, opts),
    }
}

fn approve(client: ManyClient, opts: TransactionOpt) -> Result<(), ManyError> {
    if client.id.identity.is_anonymous() {
        Err(ManyError::invalid_identity())
    } else {
        let arguments = multisig::ApproveArgs { token: opts.token };
        let response = client.call("account.multisigApprove", arguments)?;

        let payload = crate::wait_response(client, response)?;
        let _result: multisig::ApproveReturn = minicbor::decode(&payload)
            .map_err(|e| ManyError::deserialization_error(e.to_string()))?;

        info!("Approved.");

        Ok(())
    }
}

fn revoke(client: ManyClient, opts: TransactionOpt) -> Result<(), ManyError> {
    if client.id.identity.is_anonymous() {
        Err(ManyError::invalid_identity())
    } else {
        let arguments = multisig::RevokeArgs { token: opts.token };
        let response = client.call("account.multisigRevoke", arguments)?;

        let payload = crate::wait_response(client, response)?;
        let _result: multisig::RevokeReturn = minicbor::decode(&payload)
            .map_err(|e| ManyError::deserialization_error(e.to_string()))?;

        info!("Revoked.");

        Ok(())
    }
}

fn execute(client: ManyClient, opts: TransactionOpt) -> Result<(), ManyError> {
    if client.id.identity.is_anonymous() {
        Err(ManyError::invalid_identity())
    } else {
        let arguments = multisig::ExecuteArgs { token: opts.token };
        let response = client.call("account.multisigExecute", arguments)?;

        let payload = crate::wait_response(client, response)?;
        let result: ResponseMessage = minicbor::decode(&payload)
            .map_err(|e| ManyError::deserialization_error(e.to_string()))?;

        info!("Executed:");
        println!("{}", minicbor::display(&result.data?));
        Ok(())
    }
}

fn info(client: ManyClient, opts: TransactionOpt) -> Result<(), ManyError> {
    if client.id.identity.is_anonymous() {
        Err(ManyError::invalid_identity())
    } else {
        let arguments = multisig::InfoArgs { token: opts.token };
        let response = client.call("account.multisigInfo", arguments)?;

        let payload = crate::wait_response(client, response)?;
        let result: multisig::InfoReturn = minicbor::decode(&payload)
            .map_err(|e| ManyError::deserialization_error(e.to_string()))?;

        println!("{:#?}", result);
        Ok(())
    }
}

fn set_defaults(
    client: ManyClient,
    account: Address,
    opts: MultisigArgOpt,
) -> Result<(), ManyError> {
    if client.id.identity.is_anonymous() {
        Err(ManyError::invalid_identity())
    } else {
        let arguments = multisig::SetDefaultsArgs {
            account,
            threshold: opts.threshold,
            timeout_in_secs: opts.timeout.map(|d| d.as_secs()),
            execute_automatically: opts.execute_automatically,
        };
        let response = client.call("account.multisigSetDefaults", arguments)?;

        let payload = crate::wait_response(client, response)?;
        let _result: multisig::SetDefaultsReturn = minicbor::decode(&payload)
            .map_err(|e| ManyError::deserialization_error(e.to_string()))?;

        info!("Defaults set.");
        Ok(())
    }
}

pub fn multisig(client: ManyClient, opts: CommandOpt) -> Result<(), ManyError> {
    match opts.subcommand {
        SubcommandOpt::Submit {
            account,
            multisig_arg,
            subcommand,
        } => submit(client, account, multisig_arg, subcommand),
        SubcommandOpt::Approve(sub_opts) => approve(client, sub_opts),
        SubcommandOpt::Revoke(sub_opts) => revoke(client, sub_opts),
        SubcommandOpt::Execute(sub_opts) => execute(client, sub_opts),
        SubcommandOpt::Info(sub_opts) => info(client, sub_opts),
        SubcommandOpt::SetDefaults(SetDefaultsOpt {
            target_account,
            opts,
        }) => set_defaults(client, target_account, opts),
    }
}
