pub mod common;

use crate::common::{setup_with_account, AccountType, SetupWithAccount};
use many_identity::testing::identity;
use many_identity::Address;
use many_modules::account;
use many_modules::account::Role;

use many_kvstore::error;
use many_modules::kvstore::{
    GetArgs, KvStoreCommandsModuleBackend, KvStoreModuleBackend, PutArgsBuilder, QueryArgs,
};

#[test]
fn put_as_acc() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account(AccountType::KvStore);

    let put_data = PutArgsBuilder::default()
        .key(vec![1].into())
        .value(vec![2].into())
        .alternative_owner(Some(account_id))
        .build()
        .unwrap();
    let put = module_impl.put(&id, put_data.clone());
    assert!(put.is_ok());

    let get = module_impl.get(
        &Address::anonymous(),
        GetArgs {
            key: put_data.key.clone(),
        },
    );
    assert!(get.is_ok());
    assert_eq!(get.unwrap().value.unwrap(), put_data.value.into());

    let query = module_impl.query(&Address::anonymous(), QueryArgs { key: put_data.key });
    assert!(query.is_ok());
    assert_eq!(query.unwrap().owner, account_id);
}

#[test]
fn put_as_alt_invalid_addr() {
    let SetupWithAccount {
        mut module_impl,
        id,
        ..
    } = setup_with_account(AccountType::KvStore);

    let put_data = PutArgsBuilder::default()
        .key(vec![1].into())
        .value(vec![2].into())
        .alternative_owner(Some(identity(666)))
        .build()
        .unwrap();
    let put = module_impl.put(&id, put_data.clone());
    assert!(put.is_err());
    assert_eq!(put.unwrap_err().code(), error::permission_denied().code());
}

#[test]
fn put_as_alt_anon() {
    let SetupWithAccount {
        mut module_impl,
        id,
        ..
    } = setup_with_account(AccountType::KvStore);

    let put_data = PutArgsBuilder::default()
        .key(vec![1].into())
        .value(vec![2].into())
        .alternative_owner(Some(Address::anonymous()))
        .build()
        .unwrap();
    let put = module_impl.put(&id, put_data.clone());
    assert!(put.is_err());
    assert_eq!(put.unwrap_err().code(), error::anon_alt_denied().code());
}

#[test]
fn put_as_alt_subres() {
    let SetupWithAccount {
        mut module_impl,
        id,
        ..
    } = setup_with_account(AccountType::KvStore);

    let put_data = PutArgsBuilder::default()
        .key(vec![1].into())
        .value(vec![2].into())
        .alternative_owner(Some(id.with_subresource_id(2).unwrap()))
        .build()
        .unwrap();
    let put = module_impl.put(&id, put_data.clone());
    assert!(put.is_err());
    assert_eq!(
        put.unwrap_err().code(),
        error::subres_alt_unsupported().code()
    );
}

#[test]
fn put_as_sender_not_in_acc() {
    let SetupWithAccount {
        mut module_impl,
        account_id,
        ..
    } = setup_with_account(AccountType::KvStore);

    let put_data = PutArgsBuilder::default()
        .key(vec![1].into())
        .value(vec![2].into())
        .alternative_owner(Some(account_id))
        .build()
        .unwrap();
    let put = module_impl.put(&identity(666), put_data.clone());
    assert!(put.is_err());
    assert_eq!(
        put.unwrap_err().code(),
        account::errors::user_needs_role(Role::CanKvStoreWrite).code()
    );
}

#[test]
fn put_as_sender_invalid_role() {
    let SetupWithAccount {
        mut module_impl,
        account_id,
        ..
    } = setup_with_account(AccountType::KvStore);

    let put_data = PutArgsBuilder::default()
        .key(vec![1].into())
        .value(vec![2].into())
        .alternative_owner(Some(account_id))
        .build()
        .unwrap();
    // This user has `canKvStoreDisable` but not `canKvStoreWrite`
    let put = module_impl.put(&identity(3), put_data.clone());
    assert!(put.is_err());
    assert_eq!(
        put.unwrap_err().code(),
        account::errors::user_needs_role(Role::CanKvStoreWrite).code()
    );
}

#[test]
fn put_as_alt_user_in_acc_with_perm() {
    let SetupWithAccount {
        mut module_impl,
        account_id,
        ..
    } = setup_with_account(AccountType::KvStore);

    let put_data = PutArgsBuilder::default()
        .key(vec![1].into())
        .value(vec![2].into())
        .alternative_owner(Some(account_id))
        .build()
        .unwrap();
    // This user has `canKvStoreWrite`
    let put = module_impl.put(&identity(2), put_data.clone());
    assert!(put.is_ok());

    let get = module_impl.get(
        &Address::anonymous(),
        GetArgs {
            key: put_data.key.clone(),
        },
    );
    assert!(get.is_ok());
    assert_eq!(get.unwrap().value.unwrap(), put_data.value.into());

    let query = module_impl.query(&Address::anonymous(), QueryArgs { key: put_data.key });
    assert!(query.is_ok());
    assert_eq!(query.unwrap().owner, account_id);
}
