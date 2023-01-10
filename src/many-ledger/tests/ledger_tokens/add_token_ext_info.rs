use std::path::Path;
use test_macros::*;
use test_utils::cucumber::{
    AccountWorld, LedgerWorld, SomeError, SomeId, SomePermission, TokenWorld,
};
use test_utils::Setup;

use cucumber::{given, then, when, World};
use many_error::ManyError;
use many_identity::Address;
use many_ledger::migration::tokens::TOKEN_MIGRATION;
use many_ledger::module::LedgerModuleImpl;
use many_modules::ledger::extended_info::visual_logo::VisualTokenLogo;
use many_modules::ledger::extended_info::TokenExtendedInfo;
use many_modules::ledger::{LedgerTokensModuleBackend, TokenAddExtendedInfoArgs, TokenInfoArgs};
use many_types::ledger::TokenInfo;
use many_types::Memo;

#[derive(World, Debug, Default, LedgerWorld, TokenWorld, AccountWorld)]
#[world(init = Self::new)]
struct AddExtInfoWorld {
    setup: Setup,
    args: TokenAddExtendedInfoArgs,
    info: TokenInfo,
    ext_info: TokenExtendedInfo,
    account: Address,
    error: Option<ManyError>,
}

impl AddExtInfoWorld {
    fn new() -> Self {
        Self {
            setup: Setup::new_with_migrations(false, [(0, &TOKEN_MIGRATION)], true),
            ..Default::default()
        }
    }
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
#[given(expr = "a token account")]
fn given_token_account(w: &mut AddExtInfoWorld) {
    test_utils::cucumber::given_token_account(w);
}

#[given(expr = "{id} as the account owner")]
fn given_account_id_owner(w: &mut AddExtInfoWorld, id: SomeId) {
    test_utils::cucumber::given_account_id_owner(w, id);
}

#[given(expr = "{id} has {permission} permission")]
fn given_account_part_of_can_create(
    w: &mut AddExtInfoWorld,
    id: SomeId,
    permission: SomePermission,
) {
    test_utils::cucumber::given_account_part_of_can_create(w, id, permission);
}

#[given(expr = "a default token owned by {id}")]
fn create_default_token(w: &mut AddExtInfoWorld, id: SomeId) {
    test_utils::cucumber::create_default_token(w, id);
    w.args.symbol = w.info.symbol;
    refresh_token_info(w);
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