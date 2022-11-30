pub mod common;

use common::Setup;
use std::collections::{BTreeMap, BTreeSet};

use cucumber::{given, then, when, Parameter, World};
use many_error::ManyError;
use many_identity::testing::identity;
use many_identity::{Address, Identity};
use many_identity_dsa::ecdsa::generate_random_ecdsa_identity;
use many_ledger::error;
use many_modules::account;
use many_modules::account::features::{FeatureInfo, FeatureSet};
use many_modules::account::{AccountModuleBackend, CreateArgs, Role};
use many_modules::ledger::{LedgerTokensModuleBackend, TokenCreateArgs, TokenCreateReturns};
use many_types::ledger::{LedgerTokensAddressMap, TokenAmount, TokenMaybeOwner};
use std::path::Path;
use std::str::FromStr;

#[derive(World, Debug, Default)]
struct CreateWorld {
    setup: Setup,
    args: TokenCreateArgs,
    result: TokenCreateReturns,
    account: Address,
    error: Option<ManyError>,
}

#[derive(Debug, Default, Eq, Parameter, PartialEq)]
#[param(name = "id", regex = "(myself)|id ([0-9])")]
enum SomeId {
    Id(u32),
    #[default]
    Myself,
}

impl FromStr for SomeId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "myself" => Self::Myself,
            id => Self::Id(id.parse().expect("Unable to parse identity id")),
        })
    }
}

#[derive(Debug, Default, Eq, Parameter, PartialEq)]
#[param(name = "error", regex = "(unauthorized)|(missing permission)")]
enum SomeError {
    #[default]
    Unauthorized,
    MissingPermission,
}

impl FromStr for SomeError {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "unauthorized" => Self::Unauthorized,
            "missing permission" => Self::MissingPermission,
            invalid => return Err(format!("Invalid `SomeError`: {invalid}")),
        })
    }
}

// TODO: Generalize
fn create_token_account_with(
    w: &mut CreateWorld,
    id: &Address,
    roles: Option<BTreeMap<Address, BTreeSet<Role>>>,
) -> Address {
    let args = CreateArgs {
        description: Some("Account Description".into()),
        roles,
        features: FeatureSet::from_iter([
            account::features::tokens::TokenAccountLedger.as_feature()
        ]),
    };
    AccountModuleBackend::create(&mut w.setup.module_impl, id, args)
        .expect("Unable to create account")
        .id
}

fn create_token(w: &mut CreateWorld, sender: &Address) {
    w.result = LedgerTokensModuleBackend::create(&mut w.setup.module_impl, sender, w.args.clone())
        .expect("Could not create token");
}

fn fail_create_token(w: &mut CreateWorld, sender: &Address) {
    w.error = Some(
        LedgerTokensModuleBackend::create(&mut w.setup.module_impl, sender, w.args.clone())
            .expect_err("Token creation was supposed to fail, it succeeded instead."),
    );
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

#[given(expr = "myself as owner")]
fn given_token_owner(w: &mut CreateWorld) {
    w.args.owner = Some(TokenMaybeOwner::Left(w.setup.id));
}

#[given(expr = "a random owner")]
fn given_random_owner(w: &mut CreateWorld) {
    w.args.owner = Some(TokenMaybeOwner::Left(
        generate_random_ecdsa_identity().address(),
    ));
}

#[given(expr = "an anonymous owner")]
fn given_anon_owner(w: &mut CreateWorld) {
    w.args.owner = Some(TokenMaybeOwner::Left(Address::anonymous()));
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

#[given(expr = "a token account where id {int} is the owner")]
fn given_account_not_part_of(w: &mut CreateWorld, seed: u32) {
    w.account = create_token_account_with(w, &identity(seed), None);
}

#[given(expr = "a token account I'm part of as Owner")]
fn given_account_part_of_owner(w: &mut CreateWorld) {
    let id = w.setup.id;
    w.account = create_token_account_with(w, &id, None);
}

#[given(expr = "a token account id {int} is part of {word} token creation permission")]
fn given_account_part_of_can_create(w: &mut CreateWorld, seed: u32, maybe_perm: String) {
    let roles = match maybe_perm.as_str() {
        "with" => Some(BTreeMap::from([(
            identity(seed),
            BTreeSet::from_iter([Role::CanTokensCreate]),
        )])),
        "without" => None,
        _ => panic!("Invalid {maybe_perm}. Expected 'with' or 'without'"),
    };
    let id = w.setup.id;
    w.account = create_token_account_with(w, &id, roles);
}

#[given(expr = "setting the account as the owner")]
fn given_account_owner(w: &mut CreateWorld) {
    w.args.owner = Some(TokenMaybeOwner::Left(w.account));
}

#[when(expr = "the token is created")]
fn when_create_token(w: &mut CreateWorld) {
    create_token(w, &w.setup.id.clone());
}

#[when(expr = "the token is created as id {int}")]
fn when_create_token_as_id(w: &mut CreateWorld, id: u32) {
    create_token(w, &identity(id));
}

#[then(expr = "creating the token as {id} fails with {error}")]
fn then_create_token_fail_acl(w: &mut CreateWorld, id: SomeId, error: SomeError) {
    let id = match id {
        SomeId::Myself => w.setup.id,
        SomeId::Id(seed) => identity(seed),
    };

    let error = match error {
        SomeError::Unauthorized => error::unauthorized(),
        SomeError::MissingPermission => account::errors::user_needs_role(Role::CanTokensCreate),
    };
    fail_create_token(w, &id);
    assert_eq!(w.error.as_ref().expect("Expecting an error"), &error);
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

#[then(expr = "the token owner is myself")]
fn then_token_owner(w: &mut CreateWorld) {
    assert_eq!(w.setup.id, w.result.info.owner.unwrap())
}

#[then(expr = "the token owner is the account")]
fn then_token_owner_account(w: &mut CreateWorld) {
    assert_eq!(w.account, w.result.info.owner.unwrap())
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
