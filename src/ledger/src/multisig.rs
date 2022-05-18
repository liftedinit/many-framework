use crate::TargetCommandOpt;
use clap::Parser;
use many::message::ResponseMessage;
use many::server::module::account;
use many::server::module::account::features::multisig;
use many::types::ledger;
use many::{Identity, ManyError};
use many_client::ManyClient;
use minicbor::bytes::ByteVec;
use tracing::info;

#[derive(Parser)]
pub struct CommandOpt {
    #[clap(subcommand)]
    /// Multisig subcommand to execute.
    subcommand: SubcommandOpt,
}

#[derive(Parser)]
enum SubcommandOpt {
    /// Submit a new transaction to be approved.
    Submit {
        /// The account to use as the source of the multisig command.
        account: Identity,

        #[clap(subcommand)]
        subcommand: SubmitOpt,
    },

    /// Approve a transaction.
    Approve(TransactionOpt),

    /// Revoke approval of a transaction.
    Revoke(TransactionOpt),

    /// Execute a transaction.
    Execute(TransactionOpt),
}

#[derive(Parser)]
enum SubmitOpt {
    /// Send tokens to someone.
    Send(TargetCommandOpt),
}

fn parse_token(s: &str) -> Result<ByteVec, String> {
    hex::decode(s).map_err(|e| e.to_string()).map(|v| v.into())
}

#[derive(Parser)]
struct TransactionOpt {
    #[clap(parse(try_from_str=parse_token))]
    token: ByteVec,
}

fn submit_send(
    client: ManyClient,
    account: Identity,
    opts: TargetCommandOpt,
) -> Result<(), ManyError> {
    let TargetCommandOpt {
        identity,
        amount,
        symbol,
    } = opts;
    let symbol = crate::resolve_symbol(&client, symbol)?;

    if client.id.identity.is_anonymous() {
        Err(ManyError::invalid_identity())
    } else {
        let transaction = ledger::TransactionInfo::Send {
            from: account,
            to: identity,
            symbol,
            amount: ledger::TokenAmount::from(amount),
        };
        let arguments = multisig::SubmitTransactionArgs {
            account: Some(account),
            memo: None,
            transaction,
            threshold: None,
            timeout_in_secs: None,
            execute_automatically: None,
            data: None,
        };
        let response = client.call("account.multisigSubmitTransaction", arguments)?;

        let payload = crate::wait_response(client, response)?;
        let result: account::features::multisig::SubmitTransactionReturn =
            minicbor::decode(&payload)
                .map_err(|e| ManyError::deserialization_error(e.to_string()))?;

        info!(
            "Transaction Token: {}",
            hex::encode(result.token.as_slice())
        );
        Ok(())
    }
}

fn submit(client: ManyClient, account: Identity, opts: SubmitOpt) -> Result<(), ManyError> {
    match opts {
        SubmitOpt::Send(target) => submit_send(client, account, target),
    }
}

fn approve(client: ManyClient, opts: TransactionOpt) -> Result<(), ManyError> {
    if client.id.identity.is_anonymous() {
        Err(ManyError::invalid_identity())
    } else {
        let arguments = multisig::ApproveArgs { token: opts.token };
        let response = client.call("account.multisigApprove", arguments)?;

        let payload = crate::wait_response(client, response)?;
        let _result: account::features::multisig::ApproveReturn = minicbor::decode(&payload)
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
        let _result: account::features::multisig::RevokeReturn = minicbor::decode(&payload)
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

pub fn multisig(client: ManyClient, opts: CommandOpt) -> Result<(), ManyError> {
    match opts.subcommand {
        SubcommandOpt::Submit {
            account,
            subcommand,
        } => submit(client, account, subcommand),
        SubcommandOpt::Approve(sub_opts) => approve(client, sub_opts),
        SubcommandOpt::Revoke(sub_opts) => revoke(client, sub_opts),
        SubcommandOpt::Execute(sub_opts) => execute(client, sub_opts),
    }
}
