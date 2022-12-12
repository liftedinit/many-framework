use test_utils::cucumber::{SomeError, SomeId, SomePermission, TokenWorld};
use test_utils::Setup;

use cucumber::{given, then, when, World};
use many_error::ManyError;
use many_identity::testing::identity;
use many_identity::Address;
use many_ledger::migration::tokens::TOKEN_MIGRATION;
use many_ledger::module::LedgerModuleImpl;
use many_modules::ledger::{LedgerTokensModuleBackend, TokenCreateArgs};
use many_types::cbor::CborNull;
use many_types::ledger::{LedgerTokensAddressMap, TokenAmount, TokenInfo, TokenMaybeOwner};
use std::path::Path;

#[derive(World, Debug, Default)]
#[world(init = Self::new)]
struct CreateWorld {
    setup: Setup,
    args: TokenCreateArgs,
    info: TokenInfo,
    account: Address,
    error: Option<ManyError>,
}

impl CreateWorld {
    fn new() -> Self {
        Self {
            setup: Setup::new_with_migrations(false, [(0, &TOKEN_MIGRATION)], true),
            ..Default::default()
        }
    }
}

// TODO: Macro
impl TokenWorld for CreateWorld {
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

fn create_token(w: &mut CreateWorld, sender: &Address) {
    w.info = LedgerTokensModuleBackend::create(&mut w.setup.module_impl, sender, w.args.clone())
        .expect("Could not create token")
        .info;
}

fn fail_create_token(w: &mut CreateWorld, sender: &Address) {
    w.error = Some(
        LedgerTokensModuleBackend::create(&mut w.setup.module_impl, sender, w.args.clone())
            .expect_err("Token creation was supposed to fail, it succeeded instead."),
    );
}
#[given(expr = "a token account")]
fn given_token_account(w: &mut CreateWorld) {
    test_utils::cucumber::given_token_account(w);
}

#[given(expr = "{id} as the account owner")]
fn given_account_id_owner(w: &mut CreateWorld, id: SomeId) {
    test_utils::cucumber::given_account_id_owner(w, id);
}

#[given(expr = "{id} has {permission} permission")]
fn given_account_part_of_can_create(w: &mut CreateWorld, id: SomeId, permission: SomePermission) {
    test_utils::cucumber::given_account_part_of_can_create(w, id, permission);
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
    w.args.owner = Some(TokenMaybeOwner::Right(CborNull));
}

#[given(expr = "id {int} has {int} initial tokens")]
fn given_initial_distribution(w: &mut CreateWorld, id: u32, amount: u64) {
    let distribution = w
        .args
        .initial_distribution
        .get_or_insert(LedgerTokensAddressMap::default());
    distribution.insert(identity(id), TokenAmount::from(amount));
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
    assert!(w.info.symbol.is_subresource());
}

#[then(expr = "the token ticker is {word}")]
fn then_token_ticker(w: &mut CreateWorld, ticker: String) {
    assert_eq!(w.info.summary.ticker, ticker);
}

#[then(expr = "the token name is {word}")]
fn then_token_name(w: &mut CreateWorld, name: String) {
    assert_eq!(w.info.summary.name, name);
}

#[then(expr = "the token owner is {id}")]
fn then_token_owner(w: &mut CreateWorld, id: SomeId) {
    assert_eq!(id.as_address(w), w.info.owner.unwrap())
}

#[then(expr = "the owner is removed")]
fn then_token_rm_owner(w: &mut CreateWorld) {
    assert!(w.info.owner.is_none());
}

#[then(expr = "the token total supply is {int}")]
fn then_initial_supply(w: &mut CreateWorld, total_supply: u64) {
    assert_eq!(w.info.supply.total, TokenAmount::from(total_supply));
}

#[then(expr = "the token circulating supply is {int}")]
fn then_circulating_supply(w: &mut CreateWorld, circulating_supply: u64) {
    assert_eq!(
        w.info.supply.circulating,
        TokenAmount::from(circulating_supply)
    );
}

#[then(expr = "the token maximum supply has no maximum")]
fn then_maximum_supply(w: &mut CreateWorld) {
    assert_eq!(w.info.supply.maximum, None);
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
