pub mod common;

use common::many_cucumber::*;
use common::*;
use std::path::Path;
use std::str::FromStr;

use cucumber::{given, then, when, Parameter, World};
use many_error::ManyError;
use many_identity::Address;
use many_ledger::module::LedgerModuleImpl;
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
    account: Address,
    error: Option<ManyError>,
}

// TODO: Macro
impl TokenWorld for RemoveExtInfoWorld {
    fn setup_id(&self) -> Address {
        self.setup.id
    }

    fn account(&self) -> Address {
        self.account
    }

    fn info_mut(&mut self) -> &mut TokenInfo {
        &mut self.info
    }

    fn account_mut(&mut self) -> &mut Address {
        &mut self.account
    }

    fn module_impl(&mut self) -> &mut LedgerModuleImpl {
        &mut self.setup.module_impl
    }
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

fn fail_remove_ext_info_token(w: &mut RemoveExtInfoWorld, sender: &Address) {
    w.error = Some(
        LedgerTokensModuleBackend::remove_extended_info(
            &mut w.setup.module_impl,
            sender,
            w.args.clone(),
        )
        .expect_err("Token remove extended info was supposed to fail, it succeeded instead."),
    );
}

#[given(expr = "a token account")]
fn given_token_account(w: &mut RemoveExtInfoWorld) {
    many_cucumber::given_token_account(w);
}

#[given(expr = "{id} as the account owner")]
fn given_account_id_owner(w: &mut RemoveExtInfoWorld, id: SomeId) {
    many_cucumber::given_account_id_owner(w, id);
}

#[given(expr = "{id} has {permission} permission")]
fn given_account_part_of_can_create(
    w: &mut RemoveExtInfoWorld,
    id: SomeId,
    permission: SomePermission,
) {
    many_cucumber::given_account_part_of_can_create(w, id, permission);
}

fn refresh_token_info(w: &mut RemoveExtInfoWorld) {
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

#[given(expr = "a default token owned by {id}")]
fn create_default_token(w: &mut RemoveExtInfoWorld, id: SomeId) {
    many_cucumber::create_default_token(w, id);
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

#[when(expr = "I remove the {ext_info_type} as {id}")]
fn when_rm_ext_info(w: &mut RemoveExtInfoWorld, ext_info_type: ExtendedInfoType, id: SomeId) {
    w.args.extended_info = vec![AttributeRelatedIndex::from(ExtendedInfoKey::from(
        ext_info_type,
    ))];
    let id = id.as_address(w);
    w.setup
        .module_impl
        .remove_extended_info(&id, w.args.clone())
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

#[then(expr = "removing extended info {ext_info_type} as {id} fails with {error}")]
fn then_rm_ext_info_token_fail_acl(
    w: &mut RemoveExtInfoWorld,
    ext_info_type: ExtendedInfoType,
    id: SomeId,
    error: SomeError,
) {
    w.args.extended_info = vec![AttributeRelatedIndex::from(ExtendedInfoKey::from(
        ext_info_type,
    ))];
    let id = id.as_address(w);
    fail_remove_ext_info_token(w, &id);
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

    RemoveExtInfoWorld::run(
        Path::new(features).join("ledger_tokens/remove_token_ext_info.feature"),
    )
    .await;
}