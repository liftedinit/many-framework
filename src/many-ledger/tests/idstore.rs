mod common;
use crate::common::setup;
use many::{
    server::module::idstore::{self, IdStoreModuleBackend},
    Identity, ManyError,
};
use many_ledger::module::LedgerModuleImpl;

/// Setup utility for `idstore` tests
fn setup_with_args() -> (LedgerModuleImpl, Identity, idstore::StoreArgs) {
    let (id, cred_id, public_key, module_impl) = setup();
    (
        module_impl,
        id.identity,
        idstore::StoreArgs {
            address: id.identity,
            cred_id,
            public_key,
        },
    )
}

#[test]
/// Verify basic id storage
fn store() {
    let (mut module_impl, id, store_args) = setup_with_args();
    let result = module_impl.store(&id, store_args);
    assert!(result.is_ok());
}

#[test]
/// Verify we're unable to store as anonymous
fn store_anon() {
    let (mut module_impl, _, store_args) = setup_with_args();
    let result = module_impl.store(&Identity::anonymous(), store_args);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code, ManyError::invalid_identity().code);
}

#[test]
/// Verify we're unable to store when credential ID is too small
fn invalid_cred_id_too_small() {
    let (mut module_impl, id, mut store_args) = setup_with_args();
    store_args.cred_id = idstore::CredentialId(vec![1; 15].into());
    let result = module_impl.store(&id, store_args);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code,
        idstore::invalid_credential_id("".to_string()).code
    );
}

#[test]
/// Verify we're unable to store when credential ID is too long
fn invalid_cred_id_too_long() {
    let (mut module_impl, id, mut store_args) = setup_with_args();
    store_args.cred_id = idstore::CredentialId(vec![1; 1024].into());
    let result = module_impl.store(&id, store_args);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code,
        idstore::invalid_credential_id("".to_string()).code
    );
}

#[test]
/// Verify we can fetch ID from the recall phrase
fn get_from_recall_phrase() {
    let (mut module_impl, id, store_args) = setup_with_args();
    let result = module_impl.store(&id, store_args.clone());
    assert!(result.is_ok());
    let store_return = result.unwrap();

    let result =
        module_impl.get_from_recall_phrase(idstore::GetFromRecallPhraseArgs(store_return.0));
    assert!(result.is_ok());
    let get_returns = result.unwrap();
    assert_eq!(get_returns.cred_id, store_args.cred_id);
    assert_eq!(get_returns.public_key, store_args.public_key);
}

#[test]
/// Verify we can't fetch ID from an invalid recall phrase
fn get_from_invalid_recall_phrase() {
    let (mut module_impl, id, store_args) = setup_with_args();
    let result = module_impl.store(&id, store_args);
    assert!(result.is_ok());

    let result = module_impl
        .get_from_recall_phrase(idstore::GetFromRecallPhraseArgs(vec!["Foo".to_string()]));
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code,
        idstore::entry_not_found("".to_string()).code
    );
}

#[test]
/// Verify we can fetch ID from the public address
fn get_from_address() {
    let (mut module_impl, id, store_args) = setup_with_args();
    let result = module_impl.store(&id, store_args.clone());
    assert!(result.is_ok());

    let result = module_impl.get_from_address(idstore::GetFromAddressArgs(id));
    assert!(result.is_ok());
    let get_returns = result.unwrap();
    assert_eq!(get_returns.cred_id, store_args.cred_id);
    assert_eq!(get_returns.public_key, store_args.public_key);
}

#[test]
/// Verify we can't fetch ID from an invalid address
fn get_from_invalid_address() {
    let (mut module_impl, id, store_args) = setup_with_args();
    let result = module_impl.store(&id, store_args);
    assert!(result.is_ok());

    let result = module_impl.get_from_address(idstore::GetFromAddressArgs(Identity::anonymous()));
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code,
        idstore::entry_not_found("".to_string()).code
    );
}
