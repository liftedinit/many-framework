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
use many_modules::events::{EventFilter, EventKind, EventsModuleBackend, ListArgs};
use many_modules::ledger::{LedgerTokensModuleBackend, TokenInfoArgs, TokenUpdateArgs};
use many_types::ledger::{TokenInfo, TokenMaybeOwner};
use many_types::Memo;
use std::path::Path;
use std::str::FromStr;

#[derive(World, Debug, Default)]
struct UpdateWorld {
    setup: Setup,
    args: TokenUpdateArgs,
    info: TokenInfo,
    account: Address,
    error: Option<ManyError>,
}

// TODO: DRY?
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
    fn as_address(&self, w: &mut UpdateWorld) -> Address {
        match self {
            SomeId::Myself => w.setup.id,
            SomeId::Id(seed) => identity(*seed),
            SomeId::Anonymous => Address::anonymous(),
            SomeId::Random => generate_random_ecdsa_identity().address(),
            SomeId::Account => w.account,
        }
    }
}

fn fail_update_token(w: &mut UpdateWorld, sender: &Address) {
    w.error = Some(
        LedgerTokensModuleBackend::update(&mut w.setup.module_impl, sender, w.args.clone())
            .expect_err("Token update was supposed to fail, it succeeded instead."),
    );
}

// TODO: DRY
#[given(expr = "a token account")]
fn given_token_account(w: &mut UpdateWorld) {
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

// TODO: DRY
#[given(expr = "{id} as the account owner")]
fn given_account_id_owner(w: &mut UpdateWorld, id: SomeId) {
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

// TODO: DRY
#[given(expr = "{id} has {permission} permission")]
fn given_account_part_of_can_create(w: &mut UpdateWorld, id: SomeId, permission: SomePermission) {
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

// TODO: DRY
#[given(expr = "setting the account as the owner")]
fn given_account_owner(w: &mut UpdateWorld) {
    w.args.owner = Some(TokenMaybeOwner::Left(w.account));
}

// TODO: DRY
#[given(expr = "a default token owned by {id}")]
fn create_default_token(w: &mut UpdateWorld, id: SomeId) {
    let id = id.as_address(w);
    let result = LedgerTokensModuleBackend::create(
        &mut w.setup.module_impl,
        &id,
        common::default_token_create_args(Some(TokenMaybeOwner::Left(id))),
    )
    .expect("Unable to create default token");
    w.info = result.info;
    w.args.symbol = w.info.symbol;
}

#[given(expr = "a default token owned by no one")]
fn create_default_token_no_one(w: &mut UpdateWorld) {
    let result = LedgerTokensModuleBackend::create(
        &mut w.setup.module_impl,
        &w.setup.id,
        common::default_token_create_args(Some(TokenMaybeOwner::Right(()))),
    )
    .expect("Unable to create default token");
    w.info = result.info;
    w.args.symbol = w.info.symbol;
}

#[given(expr = "a new ticker {word}")]
fn given_new_ticker(w: &mut UpdateWorld, ticker: String) {
    w.args.ticker = Some(ticker);
}

#[given(expr = "a new name {word}")]
fn given_new_name(w: &mut UpdateWorld, name: String) {
    w.args.name = Some(name);
}

#[given(expr = "a new decimal {int}")]
fn given_new_decimal(w: &mut UpdateWorld, decimal: u64) {
    w.args.decimals = Some(decimal);
}

#[given(expr = "a token owner {word}")]
fn given_new_owner(w: &mut UpdateWorld, owner: Address) {
    w.args.owner = Some(TokenMaybeOwner::Left(owner));
}

#[given(expr = "a memo {string}")]
fn given_memo(w: &mut UpdateWorld, memo: String) {
    w.args.memo = Some(Memo::try_from(memo).unwrap());
}

#[given(expr = "removing the token owner")]
fn given_rm_owner(w: &mut UpdateWorld) {
    w.args.owner = Some(TokenMaybeOwner::Right(()));
}

#[when(expr = "I update the token as {id}")]
fn when_update_ticker(w: &mut UpdateWorld, id: SomeId) {
    let id = id.as_address(w);
    w.setup
        .module_impl
        .update(&id, w.args.clone())
        .expect("Unable to update token ticker");

    let res = LedgerTokensModuleBackend::info(
        &w.setup.module_impl,
        &w.setup.id,
        TokenInfoArgs {
            symbol: w.info.symbol,
            ..Default::default()
        },
    )
    .expect("Unable to fetch token info");
    w.info = res.info;
}

#[then(expr = "the token new ticker is {word}")]
fn then_new_ticker(w: &mut UpdateWorld, ticker: String) {
    assert_eq!(w.info.summary.ticker, ticker);
}

#[then(expr = "the token new name is {word}")]
fn then_new_name(w: &mut UpdateWorld, name: String) {
    assert_eq!(w.info.summary.name, name);
}

#[then(expr = "the token new decimal is {int}")]
fn then_new_decimal(w: &mut UpdateWorld, decimal: u64) {
    assert_eq!(w.info.summary.decimals, decimal);
}

#[then(expr = "the memo is {string}")]
fn then_memo(w: &mut UpdateWorld, memo: String) {
    let res = EventsModuleBackend::list(
        &w.setup.module_impl,
        ListArgs {
            filter: Some(EventFilter {
                kind: Some(vec![EventKind::TokenUpdate].into()),
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .expect("Unable to list TokenUpdate event");
    let memo = Memo::try_from(memo).unwrap();
    assert!(res.nb_events >= 1);
    // TODO: INVESTIGATE THE FAIL
    for event in res.events {
        dbg!(&event.content);
        assert!(event.content.memo().is_some());
        assert_eq!(event.content.memo().unwrap(), &memo);
    }
}

#[then(expr = "the token new owner is {id}")]
fn then_new_owner(w: &mut UpdateWorld, owner: SomeId) {
    let owner = owner.as_address(w);
    assert_eq!(w.info.owner, Some(owner));
}

#[then(expr = "the token owner is removed")]
fn then_rm_owner(w: &mut UpdateWorld) {
    assert!(w.info.owner.is_none());
}

#[then(expr = "updating the token as {id} fails with {error}")]
fn then_update_token_fail_acl(w: &mut UpdateWorld, id: SomeId, error: SomeError) {
    let id = id.as_address(w);
    fail_update_token(w, &id);
    assert_eq!(
        w.error.as_ref().expect("Expecting an error"),
        &error.as_many()
    );
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
