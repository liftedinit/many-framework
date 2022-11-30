pub mod common;

use common::*;
use std::path::Path;
use std::str::FromStr;

use cucumber::{given, then, when, Parameter, World};
use many_modules::ledger::extended_info::{ExtendedInfoKey, TokenExtendedInfo};
use many_modules::ledger::{LedgerTokensModuleBackend, TokenInfoArgs, TokenRemoveExtendedInfoArgs};
use many_types::ledger::TokenInfo;
use many_types::AttributeRelatedIndex;

#[derive(World, Debug, Default)]
struct RemoveExtInfoWorld {
    setup: Setup,
    args: TokenRemoveExtendedInfoArgs,
    info: TokenInfo,
    ext_info: TokenExtendedInfo,
}

#[derive(Debug, Default, Parameter)]
#[param(name = "ext_info_type", regex = "memo|logo")]
enum ExtendedInfoType {
    #[default]
    Memo,
    VisualLogo,
}

impl FromStr for ExtendedInfoType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "memo" => Self::Memo,
            "logo" => Self::VisualLogo,
            invalid => return Err(format!("Invalid `ExtendedInfoType`: {invalid}")),
        })
    }
}

impl From<ExtendedInfoType> for ExtendedInfoKey {
    fn from(value: ExtendedInfoType) -> Self {
        match value {
            ExtendedInfoType::Memo => ExtendedInfoKey::Memo,
            ExtendedInfoType::VisualLogo => ExtendedInfoKey::VisualLogo,
        }
    }
}

fn refresh_token_info(w: &mut RemoveExtInfoWorld) {
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
fn create_default_token(w: &mut RemoveExtInfoWorld) {
    let result = w
        .setup
        .module_impl
        .create(&w.setup.id, common::default_token_create_args())
        .expect("Unable to create default token");
    w.info = result.info;
    w.args.symbol = w.info.symbol;

    refresh_token_info(w);
}

#[given(expr = "the token has a memo")]
fn given_has_memo(w: &mut RemoveExtInfoWorld) {
    assert!(w.ext_info.memo().is_some());
}

#[given(expr = "the token has a logo")]
fn given_has_logo(w: &mut RemoveExtInfoWorld) {
    assert!(w.ext_info.visual_logo().is_some());
}

#[when(expr = "I remove the {ext_info_type}")]
fn when_rm_ext_info(w: &mut RemoveExtInfoWorld, ext_info_type: ExtendedInfoType) {
    w.args.extended_info = vec![AttributeRelatedIndex::from(ExtendedInfoKey::from(
        ext_info_type,
    ))];
    w.setup
        .module_impl
        .remove_extended_info(&w.setup.id, w.args.clone())
        .expect("Unable to remove extended info");

    refresh_token_info(w);
}

#[then(expr = "the token has no memo")]
fn then_no_memo(w: &mut RemoveExtInfoWorld) {
    assert!(w.ext_info.memo().is_none());
}

#[then(expr = "the token has no logo")]
fn then_no_logo(w: &mut RemoveExtInfoWorld) {
    assert!(w.ext_info.visual_logo().is_none());
}

#[tokio::main]
async fn main() {
    // Support both Cargo and Bazel paths
    let features = ["tests/features", "src/many-ledger/tests/features"]
        .into_iter()
        .find(|&p| Path::new(p).exists())
        .expect("Cucumber test features not found");

    RemoveExtInfoWorld::run(
        Path::new(features).join("ledger_tokens/remove_token_ext_info.feature"),
    )
    .await;
}
