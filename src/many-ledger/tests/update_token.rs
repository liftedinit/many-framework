pub mod common;
use common::Setup;

use cucumber::{given, then, when, World};
use many_identity::Address;
use many_modules::ledger::{LedgerTokensModuleBackend, TokenInfoArgs, TokenUpdateArgs};
use many_types::ledger::TokenInfo;
use std::path::Path;

#[derive(World, Debug, Default)]
struct UpdateWorld {
    setup: Setup,
    args: TokenUpdateArgs,
    result: TokenInfo,
}

#[given(expr = "a default token")]
fn create_default_token(w: &mut UpdateWorld) {
    let result = w
        .setup
        .module_impl
        .create(&w.setup.id, common::default_token_create_args())
        .expect("Unable to create default token");
    w.result = result.info;
}

#[given(expr = "a new token ticker {word}")]
fn given_new_ticker(w: &mut UpdateWorld, ticker: String) {
    w.args.ticker = Some(ticker);
}

#[given(expr = "a new token name {word}")]
fn given_new_name(w: &mut UpdateWorld, name: String) {
    w.args.name = Some(name);
}

#[given(expr = "a new token decimal {int}")]
fn given_new_decimal(w: &mut UpdateWorld, decimal: u64) {
    w.args.decimals = Some(decimal);
}

#[given(expr = "a new token owner {word}")]
fn given_new_owner(w: &mut UpdateWorld, owner: Address) {
    w.args.owner = Some(Some(owner));
}

#[given(expr = "removing the token owner")]
fn given_rm_owner(w: &mut UpdateWorld) {
    w.args.owner = Some(None);
}

#[when(expr = "I update the token")]
fn when_update_ticker(w: &mut UpdateWorld) {
    w.setup
        .module_impl
        .update(&w.setup.id, w.args.clone())
        .expect("Unable to update token ticker");

    let res = w
        .setup
        .module_impl
        .info(
            &w.setup.id,
            TokenInfoArgs {
                symbol: w.result.symbol,
                ..Default::default()
            },
        )
        .expect("Unable to fetch token info");
    w.result = res.info;
}

#[then(expr = "the token new ticker is {word}")]
fn then_new_ticker(w: &mut UpdateWorld, ticker: String) {
    assert_eq!(w.result.summary.ticker, ticker);
}

#[then(expr = "the token new name is {word}")]
fn then_new_name(w: &mut UpdateWorld, name: String) {
    assert_eq!(w.result.summary.name, name);
}

#[then(expr = "the token new decimal is {int}")]
fn then_new_decimal(w: &mut UpdateWorld, decimal: u64) {
    assert_eq!(w.result.summary.decimals, decimal);
}

#[then(expr = "the token new owner is {word}")]
fn then_new_owner(w: &mut UpdateWorld, owner: Address) {
    assert_eq!(w.result.owner, Some(owner));
}

#[then(expr = "the token owner is removed")]
fn then_rm_owner(w: &mut UpdateWorld) {
    assert!(w.result.owner.is_none());
}

#[tokio::main]
async fn main() {
    // Support both Cargo and Bazel paths
    let features = ["tests/features", "src/many-ledger/tests/features"]
        .into_iter()
        .find(|&p| Path::new(p).exists())
        .expect("Cucumber test features not found");

    UpdateWorld::run(Path::new(features).join("ledger_tokens/update_token.feature")).await;
}
