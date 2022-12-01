pub mod common;

use common::*;
use std::collections::{BTreeMap, BTreeSet};

use cucumber::{given, then, when, Parameter, World};
use many_error::ManyError;
use many_identity::testing::identity;
use many_identity::{Address, Identity};
use many_identity_dsa::ecdsa::generate_random_ecdsa_identity;
use many_modules::account;
use many_modules::account::features::{FeatureInfo, FeatureSet};
use many_modules::account::{
    AccountModuleBackend, AddRolesArgs, CreateArgs, RemoveRolesArgs, Role,
};
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
#[param(
    name = "id",
    regex = "(myself)|id ([0-9])|(random)|(anonymous)|(the account)"
)]
pub enum SomeId {
    Id(u32),
    #[default]
    Myself,
    Anonymous,
    Random,
    Account,
}

impl FromStr for SomeId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "myself" => Self::Myself,
            "anonymous" => Self::Anonymous,
            "random" => Self::Random,
            "the account" => Self::Account,
            id => Self::Id(id.parse().expect("Unable to parse identity id")),
        })
    }
}

impl SomeId {
    fn as_address(&self, w: &mut CreateWorld) -> Address {
        match self {
            SomeId::Myself => w.setup.id,
            SomeId::Id(seed) => identity(*seed),
            SomeId::Anonymous => Address::anonymous(),
            SomeId::Random => generate_random_ecdsa_identity().address(),
            SomeId::Account => w.account,
        }
    }
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

#[given(expr = "{id} as owner")]
fn given_token_owner(w: &mut CreateWorld, id: SomeId) {
    w.args.owner = Some(TokenMaybeOwner::Left(id.as_address(w)));
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

#[given(expr = "a token account")]
fn given_token_account(w: &mut CreateWorld) {
    let account = AccountModuleBackend::create(
        &mut w.setup.module_impl,
        &w.setup.id,
        CreateArgs {
            description: Some("Token Account".into()),
            features: FeatureSet::from_iter([
                account::features::tokens::TokenAccountLedger.as_feature()
            ]),
            ..Default::default()
        },
    )
    .expect("Unable to create account");
    w.account = account.id
}

#[given(expr = "{id} as the account owner")]
fn given_account_id_owner(w: &mut CreateWorld, id: SomeId) {
    let id = id.as_address(w);
    AccountModuleBackend::add_roles(
        &mut w.setup.module_impl,
        &w.setup.id,
        AddRolesArgs {
            account: w.account,
            roles: BTreeMap::from_iter([(id, BTreeSet::from([Role::Owner]))]),
        },
    )
    .expect("Unable to add role to account");

    if id != w.setup.id {
        AccountModuleBackend::remove_roles(
            &mut w.setup.module_impl,
            &w.setup.id,
            RemoveRolesArgs {
                account: w.account,
                roles: BTreeMap::from_iter([(w.setup.id, BTreeSet::from([Role::Owner]))]),
            },
        )
        .expect("Unable to remove myself as account owner");
    }
}

#[given(expr = "{id} has {permission} permission")]
fn given_account_part_of_can_create(w: &mut CreateWorld, id: SomeId, permission: SomePermission) {
    let id = id.as_address(w);
    AccountModuleBackend::add_roles(
        &mut w.setup.module_impl,
        &w.setup.id,
        AddRolesArgs {
            account: w.account,
            roles: BTreeMap::from([(id, BTreeSet::from_iter([permission.as_role()]))]),
        },
    )
    .expect("Unable to add role to account");
}

#[given(expr = "setting the account as the owner")]
fn given_account_owner(w: &mut CreateWorld) {
    w.args.owner = Some(TokenMaybeOwner::Left(w.account));
}

#[when(expr = "the token is created as {id}")]
fn when_create_token(w: &mut CreateWorld, id: SomeId) {
    let id = id.as_address(w);
    create_token(w, &id);
}

#[then(expr = "creating the token as {id} fails with {error}")]
fn then_create_token_fail_acl(w: &mut CreateWorld, id: SomeId, error: SomeError) {
    let id = id.as_address(w);
    fail_create_token(w, &id);
    assert_eq!(
        w.error.as_ref().expect("Expecting an error"),
        &error.as_many()
    );
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

#[then(expr = "the token owner is {id}")]
fn then_token_owner(w: &mut CreateWorld, id: SomeId) {
    assert_eq!(id.as_address(w), w.result.info.owner.unwrap())
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
