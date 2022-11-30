pub mod common;
use common::Setup;

use cucumber::{given, then, when, World};
use many_identity::testing::identity;
use many_identity::Address;
use many_modules::ledger::{LedgerTokensModuleBackend, TokenCreateArgs, TokenCreateReturns};
use many_types::ledger::{LedgerTokensAddressMap, TokenAmount, TokenMaybeOwner};
use std::path::Path;

#[derive(World, Debug, Default)]
struct CreateWorld {
    setup: Setup,
    args: TokenCreateArgs,
    result: TokenCreateReturns,
}

#[given(expr = "a name {word}")]
fn given_token_name(w: &mut CreateWorld, name: String) {
    w.args.summary.name = name;
}

#[given(expr = "a ticker {word}")]
fn given_token_ticker(w: &mut CreateWorld, ticker: String) {
    w.args.summary.ticker = ticker;
}

#[given(expr = "a decimals of {int}")]
fn given_token_decimals(w: &mut CreateWorld, decimals: u64) {
    w.args.summary.decimals = decimals;
}

#[given(expr = "an owner {word}")]
fn given_token_owner(w: &mut CreateWorld, owner: Address) {
    w.args.owner = Some(TokenMaybeOwner::Left(owner));
}

#[given(expr = "no owner")]
fn given_token_owner_none(w: &mut CreateWorld) {
    w.args.owner = None;
}

#[given(expr = "removing the owner")]
fn given_token_rm_owner(w: &mut CreateWorld) {
    w.args.owner = Some(TokenMaybeOwner::Right(()));
}

#[given(expr = "id {int} has {int} initial tokens")]
fn given_initial_distribution(w: &mut CreateWorld, id: u32, amount: u64) {
    let distribution = w
        .args
        .initial_distribution
        .get_or_insert(LedgerTokensAddressMap::default());
    distribution.insert(identity(id), TokenAmount::from(amount));
}

#[when(expr = "the token is created")]
fn when_create_token(w: &mut CreateWorld) {
    w.result = w
        .setup
        .module_impl
        .create(&w.setup.id, w.args.clone())
        .expect("Could not create token");
}

#[then(expr = "the token symbol is a subresource")]
fn then_token_symbol(w: &mut CreateWorld) {
    assert!(w.result.info.symbol.is_subresource());
}

#[then(expr = "the token ticker is {word}")]
fn then_token_ticker(w: &mut CreateWorld, ticker: String) {
    assert_eq!(w.result.info.summary.ticker, ticker);
}

#[then(expr = "the token name is {word}")]
fn then_token_name(w: &mut CreateWorld, name: String) {
    assert_eq!(w.result.info.summary.name, name);
}

#[then(expr = "the token owner is {word}")]
fn then_token_owner(w: &mut CreateWorld, owner: Address) {
    if let Some(some_owner) = w.result.info.owner {
        assert_eq!(owner, some_owner);
    }
}

#[then(expr = "the sender is the owner")]
fn then_token_no_owner(w: &mut CreateWorld) {
    assert_eq!(w.result.info.owner, w.setup.id);
}

#[then(expr = "the owner is removed")]
fn then_token_rm_owner(w: &mut CreateWorld) {
    assert!(w.result.info.owner.is_none());
}

#[then(expr = "the token total supply is {int}")]
fn then_initial_supply(w: &mut CreateWorld, total_supply: u64) {
    assert_eq!(w.result.info.supply.total, TokenAmount::from(total_supply));
}

#[then(expr = "the token circulating supply is {int}")]
fn then_circulating_supply(w: &mut CreateWorld, circulating_supply: u64) {
    assert_eq!(
        w.result.info.supply.circulating,
        TokenAmount::from(circulating_supply)
    );
}

#[then(expr = "the token maximum supply has no maximum")]
fn then_maximum_supply(w: &mut CreateWorld) {
    assert_eq!(w.result.info.supply.maximum, None);
}

#[tokio::main]
async fn main() {
    // Support both Cargo and Bazel paths
    let features = ["tests/features", "src/many-ledger/tests/features"]
        .into_iter()
        .find(|&p| Path::new(p).exists())
        .expect("Cucumber test features not found");

    CreateWorld::run(Path::new(features).join("ledger_tokens/create_token.feature")).await;
}
