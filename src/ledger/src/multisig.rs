use crate::TargetCommandOpt;
use clap::Parser;
use many::server::module::account;
use many::types::ledger;
use many::{Identity, ManyError};
use many::server::module::async::attributes::AsyncAttribute;
use many_client::ManyClient;
use tracing::info;

#[derive(Parser)]
pub struct CommandOpt {
    /// The account to use as the source of the multisig command.
    account: Identity,

    #[clap(subcommand)]
    /// Multisig subcommand to execute.
    subcommand: SubcommandOpt,
}

#[derive(Parser)]
enum SubcommandOpt {
    Submit(SubmitOpt),
}

#[derive(Parser)]
enum SubmitOpt {
    Send { TargetCommandOpt },
}

fn submit_send(client: ManyClient, account: Identity, opts: TargetCommandOpt) -> Result<(), ManyError> {
    let TargetCommandOpt {
        identity,
        amount,
        symbol,
    } = opts;
    let symbol = crate::resolve_symbol(&client, symbol)?;

    if client.id.identity.is_anonymous() {
        Err(ManyError::invalid_identity())
    } else {
        let info = ledger::TransactionInfo::Send {
            from: identity,
            to: Default::default(),
            symbol,
            amount: ledger::TokenAmount::from(amount),
        };
        let arguments = account::features::multisig::SubmitTransactionArg {
            account,
            memo: None,
            transaction: (),
            threshold: None,
            timeout_in_secs: None,
            execute_automatically: None
        };
        let many::message::ResponseMessage {
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

fn submit(client: ManyClient, opts: SubmitOpt) -> Result<(), ManyError> {
    match opts {
        SubmitOpt::Send(send_opts) => submit_send(client, send_opts),
    }
}

pub fn multisig(client: ManyClient, opts: CommandOpt) -> Result<(), ManyError> {
    match opts.subcommand {
        SubcommandOpt::Submit(sub_opts) => submit(client, sub_opts),
    }
}
