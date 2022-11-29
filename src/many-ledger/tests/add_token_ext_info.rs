pub mod common;

use common::*;
use std::path::Path;

use cucumber::{given, then, when, World};
use many_modules::ledger::extended_info::visual_logo::VisualTokenLogo;
use many_modules::ledger::extended_info::TokenExtendedInfo;
use many_modules::ledger::{LedgerTokensModuleBackend, TokenAddExtendedInfoArgs, TokenInfoArgs};
use many_types::ledger::TokenInfo;
use many_types::Memo;

#[derive(World, Debug, Default)]
struct AddExtInfoWorld {
    setup: Setup,
    args: TokenAddExtendedInfoArgs,
    info: TokenInfo,
    ext_info: TokenExtendedInfo,
}

fn refresh_token_info(w: &mut AddExtInfoWorld) {
    let result = w
        .setup
        .module_impl
        .info(
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

#[given(expr = "a default token")]
fn create_default_token(w: &mut AddExtInfoWorld) {
    let result = w
        .setup
        .module_impl
        .create(&w.setup.id, common::default_token_create_args())
        .expect("Unable to create default token");
    w.info = result.info;

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

#[when(expr = "I add the extended info to the token")]
fn add_ext_info(w: &mut AddExtInfoWorld) {
    w.setup
        .module_impl
        .add_extended_info(&w.setup.id, w.args.clone())
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
