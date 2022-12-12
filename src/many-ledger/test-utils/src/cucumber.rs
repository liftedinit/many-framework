use cucumber::Parameter;
use many_error::ManyError;
use many_identity::testing::identity;
use many_identity::{Address, Identity};
use many_identity_dsa::ecdsa::generate_random_ecdsa_identity;
use many_ledger::error;
use many_ledger::module::LedgerModuleImpl;
use many_modules::account;
use many_modules::account::features::{FeatureInfo, FeatureSet};
use many_modules::account::{
    AccountModuleBackend, AddRolesArgs, CreateArgs, RemoveRolesArgs, Role,
};
use many_modules::ledger::LedgerTokensModuleBackend;
use many_types::cbor::CborNull;
use many_types::ledger::{TokenInfo, TokenMaybeOwner};
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

pub trait LedgerWorld {
    fn setup_id(&self) -> Address;
    fn module_impl(&mut self) -> &mut LedgerModuleImpl;
}

pub trait AccountWorld {
    fn account(&self) -> Address;
    fn account_mut(&mut self) -> &mut Address;
}

pub trait TokenWorld {
    fn info_mut(&mut self) -> &mut TokenInfo;
}

#[derive(Debug, Default, Eq, Parameter, PartialEq)]
#[param(
    name = "id",
    regex = "(myself)|id ([0-9])|(random)|(anonymous)|(the account)|(no one)"
)]
pub enum SomeId {
    Id(u32),
    #[default]
    Myself,
    Anonymous,
    Random,
    Account,
    NoOne,
}

impl FromStr for SomeId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "myself" => Self::Myself,
            "anonymous" => Self::Anonymous,
            "random" => Self::Random,
            "the account" => Self::Account,
            "no one" => Self::NoOne,
            id => Self::Id(id.parse().expect("Unable to parse identity id")),
        })
    }
}

impl SomeId {
    pub fn as_address<T: LedgerWorld + AccountWorld>(&self, w: &T) -> Address {
        match self {
            SomeId::Myself => w.setup_id(),
            SomeId::Id(seed) => identity(*seed),
            SomeId::Anonymous => Address::anonymous(),
            SomeId::Random => generate_random_ecdsa_identity().address(),
            SomeId::Account => w.account(),
            _ => unimplemented!(),
        }
    }

    pub fn as_maybe_address<T: LedgerWorld + AccountWorld>(&self, w: &T) -> Option<Address> {
        match self {
            SomeId::NoOne => None,
            _ => Some(self.as_address(w)),
        }
    }
}

#[derive(Debug, Default, Eq, Parameter, PartialEq)]
#[param(
    name = "error",
    regex = "(unauthorized)|missing permission ([a-z ]+)|(immutable)"
)]
pub enum SomeError {
    #[default]
    Unauthorized,
    MissingPermission(SomePermission),
    Immutable,
}

impl FromStr for SomeError {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "unauthorized" => Self::Unauthorized,
            "immutable" => Self::Immutable,
            permission => Self::MissingPermission(
                SomePermission::from_str(permission)
                    .expect("Unable to convert string to permission"),
            ),
        })
    }
}
#[derive(Debug, Default, Eq, Parameter, PartialEq)]
#[param(
    name = "permission",
    regex = "(token creation)|(token mint)|(token update)|(token add extended info)|(token remove extended info)"
)]
pub enum SomePermission {
    #[default]
    Create,
    Update,
    AddExtInfo,
    RemoveExtInfo,
    Mint,
}

impl FromStr for SomePermission {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "token creation" => Self::Create,
            "token mint" => Self::Mint,
            "token update" => Self::Update,
            "token add extended info" => Self::AddExtInfo,
            "token remove extended info" => Self::RemoveExtInfo,
            invalid => return Err(format!("Invalid `SomeError`: {invalid}")),
        })
    }
}

impl SomeError {
    pub fn as_many(&self) -> ManyError {
        match self {
            SomeError::Unauthorized => error::unauthorized(),
            SomeError::MissingPermission(permission) => {
                account::errors::user_needs_role(permission.as_role())
            }
            SomeError::Immutable => ManyError::unknown("Unable to update, this token is immutable"), // TODO: Custom error
        }
    }
}

impl SomePermission {
    pub fn as_role(&self) -> Role {
        match self {
            SomePermission::Create => Role::CanTokensCreate,
            SomePermission::Mint => Role::CanTokensMint,
            SomePermission::Update => Role::CanTokensUpdate,
            SomePermission::AddExtInfo => Role::CanTokensAddExtendedInfo,
            SomePermission::RemoveExtInfo => Role::CanTokensRemoveExtendedInfo,
        }
    }
}

pub fn given_token_account<T: LedgerWorld + AccountWorld>(w: &mut T) {
    let sender = w.setup_id();
    let account = AccountModuleBackend::create(
        w.module_impl(),
        &sender,
        CreateArgs {
            description: Some("Token Account".into()),
            features: FeatureSet::from_iter([
                account::features::tokens::TokenAccountLedger.as_feature()
            ]),
            ..Default::default()
        },
    )
    .expect("Unable to create account");
    *w.account_mut() = account.id
}

pub fn given_account_id_owner<T: LedgerWorld + AccountWorld>(w: &mut T, id: SomeId) {
    let id = id.as_address(w);
    let sender = w.setup_id();
    let account = w.account();
    AccountModuleBackend::add_roles(
        w.module_impl(),
        &sender,
        AddRolesArgs {
            account,
            roles: BTreeMap::from_iter([(id, BTreeSet::from([Role::Owner]))]),
        },
    )
    .expect("Unable to add role to account");

    if id != w.setup_id() {
        let account = w.account();
        AccountModuleBackend::remove_roles(
            w.module_impl(),
            &sender,
            RemoveRolesArgs {
                account,
                roles: BTreeMap::from_iter([(sender, BTreeSet::from([Role::Owner]))]),
            },
        )
        .expect("Unable to remove myself as account owner");
    }
}

pub fn given_account_part_of_can_create<T: LedgerWorld + AccountWorld>(
    w: &mut T,
    id: SomeId,
    permission: SomePermission,
) {
    let id = id.as_address(w);
    let sender = w.setup_id();
    let account = w.account();
    AccountModuleBackend::add_roles(
        w.module_impl(),
        &sender,
        AddRolesArgs {
            account,
            roles: BTreeMap::from([(id, BTreeSet::from_iter([permission.as_role()]))]),
        },
    )
    .expect("Unable to add role to account");
}

pub fn create_default_token<T: TokenWorld + LedgerWorld + AccountWorld>(w: &mut T, id: SomeId) {
    let (id, owner) = if let Some(id) = id.as_maybe_address(w) {
        (id, TokenMaybeOwner::Left(id))
    } else {
        (w.setup_id(), TokenMaybeOwner::Right(CborNull))
    };
    let result = LedgerTokensModuleBackend::create(
        w.module_impl(),
        &id,
        crate::default_token_create_args(Some(owner)),
    )
    .expect("Unable to create default token");
    *w.info_mut() = result.info;
}
