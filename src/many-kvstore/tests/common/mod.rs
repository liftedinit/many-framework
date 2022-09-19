use many_identity::testing::identity;
use many_identity::{Address, Identity};
use many_identity_dsa::ecdsa::generate_random_ecdsa_identity;
use many_kvstore::module::KvStoreModuleImpl;
use many_modules::account;
use many_modules::account::features::FeatureInfo;
use many_modules::account::{AccountModuleBackend, Role};
use std::collections::BTreeMap;

pub struct Setup {
    pub module_impl: KvStoreModuleImpl,
    pub id: Address,
}

impl Default for Setup {
    fn default() -> Self {
        Self::new(false)
    }
}

impl Setup {
    pub fn new(blockchain: bool) -> Self {
        let id = generate_random_ecdsa_identity();
        let content = std::fs::read_to_string("../../staging/kvstore_state.json5")
            .or_else(|_| std::fs::read_to_string("staging/kvstore_state.json5"))
            .unwrap();
        let state = json5::from_str(&content).unwrap();
        Self {
            module_impl: KvStoreModuleImpl::new(state, tempfile::tempdir().unwrap(), blockchain)
                .unwrap(),
            id: id.address(),
        }
    }
}

pub fn setup() -> Setup {
    Setup::default()
}

#[derive(Clone)]
#[non_exhaustive]
pub enum AccountType {
    KvStore,
}

fn create_account_args(account_type: AccountType) -> account::CreateArgs {
    let (roles, features) = match account_type {
        AccountType::KvStore => {
            let roles = Some(BTreeMap::from_iter([
                (identity(2), [Role::CanKvStoreWrite].into()),
                (identity(3), [Role::CanKvStoreDisable].into()),
            ]));
            let features = account::features::FeatureSet::from_iter([
                account::features::kvstore::AccountKvStore.as_feature(),
            ]);
            (roles, features)
        }
    };

    account::CreateArgs {
        description: Some("Foobar".to_string()),
        roles,
        features,
    }
}

pub struct SetupWithArgs {
    pub module_impl: KvStoreModuleImpl,
    pub id: Address,
    pub args: account::CreateArgs,
}

pub fn setup_with_args(account_type: AccountType) -> SetupWithArgs {
    let setup = Setup::default();
    let args = create_account_args(account_type);

    SetupWithArgs {
        module_impl: setup.module_impl,
        id: setup.id,
        args,
    }
}

pub struct SetupWithAccount {
    pub module_impl: KvStoreModuleImpl,
    pub id: Address,
    pub account_id: Address,
}

pub fn setup_with_account(account_type: AccountType) -> SetupWithAccount {
    let SetupWithArgs {
        mut module_impl,
        id,
        args,
    } = setup_with_args(account_type);
    let account = module_impl.create(&id, args).unwrap();
    SetupWithAccount {
        module_impl,
        id,
        account_id: account.id,
    }
}
