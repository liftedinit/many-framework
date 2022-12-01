pub mod common;

use common::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::str::FromStr;

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
use many_modules::ledger::extended_info::visual_logo::VisualTokenLogo;
use many_modules::ledger::extended_info::TokenExtendedInfo;
use many_modules::ledger::{LedgerTokensModuleBackend, TokenAddExtendedInfoArgs, TokenInfoArgs};
use many_types::ledger::{TokenInfo, TokenMaybeOwner};
use many_types::Memo;

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
    fn as_address(&self, w: &mut AddExtInfoWorld) -> Address {
        match self {
            SomeId::Myself => w.setup.id,
            SomeId::Id(seed) => identity(*seed),
            SomeId::Anonymous => Address::anonymous(),
            SomeId::Random => generate_random_ecdsa_identity().address(),
            SomeId::Account => w.account,
        }
    }
}

#[derive(World, Debug, Default)]
struct AddExtInfoWorld {
    setup: Setup,
    args: TokenAddExtendedInfoArgs,
    info: TokenInfo,
    ext_info: TokenExtendedInfo,
    account: Address,
    error: Option<ManyError>,
}

fn refresh_token_info(w: &mut AddExtInfoWorld) {
    let result = LedgerTokensModuleBackend::info(
        &w.setup.module_impl,
        &w.setup.id,
        TokenInfoArgs {
            symbol: w.info.symbol,
            ..Default::default()
        },
    )
    .expect("Unable to query token info");
    w.info = result.info;
    w.ext_info = result.extended_info;
}

fn fail_add_ext_info_token(w: &mut AddExtInfoWorld, sender: &Address) {
    w.error = Some(
        LedgerTokensModuleBackend::add_extended_info(
            &mut w.setup.module_impl,
            sender,
            w.args.clone(),
        )
        .expect_err("Token add extended info was supposed to fail, it succeeded instead."),
    );
}

// TODO: DRY
#[given(expr = "a token account")]
fn given_token_account(w: &mut AddExtInfoWorld) {
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
fn given_account_id_owner(w: &mut AddExtInfoWorld, id: SomeId) {
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
fn given_account_part_of_can_create(
    w: &mut AddExtInfoWorld,
    id: SomeId,
    permission: SomePermission,
) {
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
#[given(expr = "a default token owned by {id}")]
fn create_default_token(w: &mut AddExtInfoWorld, id: SomeId) {
    let id = id.as_address(w);
    let result = LedgerTokensModuleBackend::create(
        &mut w.setup.module_impl,
        &id,
        common::default_token_create_args(Some(TokenMaybeOwner::Left(id))),
    )
    .expect("Unable to create default token");
    w.info = result.info;
    w.args.symbol = w.info.symbol;

    refresh_token_info(w);
}

// TODO: DRY
#[given(expr = "a default token owned by no one")]
fn create_default_token_no_one(w: &mut AddExtInfoWorld) {
    let result = LedgerTokensModuleBackend::create(
        &mut w.setup.module_impl,
        &w.setup.id,
        common::default_token_create_args(Some(TokenMaybeOwner::Right(()))),
    )
    .expect("Unable to create default token");
    w.info = result.info;
    w.args.symbol = w.info.symbol;
}

#[given(expr = "a memo {string}")]
fn given_memo(w: &mut AddExtInfoWorld, memo: String) {
    w.args.extended_info = TokenExtendedInfo::new()
        .with_memo(Memo::try_from(memo).expect("Unable to create memo"))
        .expect("Unable to set extended info memo");
}

#[given(expr = "an unicode logo {word}")]
fn given_unicode_logo(w: &mut AddExtInfoWorld, unicode_char: char) {
    let mut logo = VisualTokenLogo::new();
    logo.unicode_front(unicode_char);
    w.args.extended_info = TokenExtendedInfo::new()
        .with_visual_logo(logo)
        .expect("Unable to set extended info logo");
}

#[given(expr = "a {word} image logo {string}")]
fn given_string_logo(w: &mut AddExtInfoWorld, content_type: String, data: String) {
    let mut logo = VisualTokenLogo::new();
    logo.image_front(content_type, data.into_bytes());
    w.args.extended_info = TokenExtendedInfo::new()
        .with_visual_logo(logo)
        .expect("Unable to set extended info logo");
}

#[when(expr = "I add the extended info to the token as {id}")]
fn add_ext_info(w: &mut AddExtInfoWorld, id: SomeId) {
    let id = id.as_address(w);
    w.setup
        .module_impl
        .add_extended_info(&id, w.args.clone())
        .expect("Unable to add extended info");

    refresh_token_info(w);
}

#[then(expr = "the token has the memo {string}")]
fn then_has_memo(w: &mut AddExtInfoWorld, memo: String) {
    assert!(w.ext_info.memo().is_some());
    assert_eq!(w.ext_info.memo().unwrap(), &Memo::try_from(memo).unwrap());
}

#[then(expr = "the token has the unicode logo {word}")]
fn then_has_unicode_logo(w: &mut AddExtInfoWorld, unicode_char: char) {
    assert!(w.ext_info.visual_logo().is_some());
    let mut logo = VisualTokenLogo::new();
    logo.unicode_front(unicode_char);
    assert_eq!(w.ext_info.visual_logo().unwrap(), &logo);
}

#[then(expr = "the token has the {word} image logo {string}")]
fn then_has_image_logo(w: &mut AddExtInfoWorld, content_type: String, data: String) {
    assert!(w.ext_info.visual_logo().is_some());
    let mut logo = VisualTokenLogo::new();
    logo.image_front(content_type, data.into_bytes());
    assert_eq!(w.ext_info.visual_logo().unwrap(), &logo);
}

#[then(expr = "adding extended info to the token as {id} fails with {error}")]
fn then_add_ext_info_token_fail_acl(w: &mut AddExtInfoWorld, id: SomeId, error: SomeError) {
    let id = id.as_address(w);
    fail_add_ext_info_token(w, &id);
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

    AddExtInfoWorld::run(Path::new(features).join("ledger_tokens/add_token_ext_info.feature"))
        .await;
}
