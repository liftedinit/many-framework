mod common;

use crate::common::setup;
use many::server::module::account::features::{FeatureInfo, TryCreateFeature};
use many::server::module::account::{self, AccountModuleBackend};
use many::types::identity::testing::identity;
use many::types::VecOrSingle;
use many::Identity;
use many_ledger::module::LedgerModuleImpl;
use std::collections::{BTreeMap, BTreeSet};

fn setup_with_args() -> (LedgerModuleImpl, Identity, account::CreateArgs) {
    let (id, _, _, module_impl) = setup();
    (
        module_impl,
        id.identity,
        account::CreateArgs {
            description: Some("Foobar".to_string()),
            roles: Some(BTreeMap::from_iter([
                (
                    identity(2),
                    BTreeSet::from_iter([account::Role::CanMultisigApprove]),
                ),
                (
                    identity(3),
                    BTreeSet::from_iter([account::Role::CanMultisigSubmit]),
                ),
            ])),
            features: account::features::FeatureSet::from_iter([
                account::features::multisig::MultisigAccountFeature::default().as_feature(),
            ]),
        },
    )
}

#[test]
/// Verify we can create an account
fn create() {
    let (mut module_impl, id, create_args) = setup_with_args();
    let result = module_impl.create(&id, create_args);
    assert!(result.is_ok());
}

#[test]
/// Verify we can't create an account with roles unsupported by feature
fn create_invalid_role() {
    let (mut module_impl, id, mut create_args) = setup_with_args();
    if let Some(roles) = create_args.roles.as_mut() {
        roles.insert(
            identity(4),
            BTreeSet::from_iter([account::Role::CanLedgerTransact]),
        );
    }
    let result = module_impl.create(&id, create_args);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code,
        account::errors::unknown_role("").code,
    );
}

#[test]
/// Verify we can change the account description
fn set_description() {
    let (mut module_impl, id, create_args) = setup_with_args();
    let account = module_impl.create(&id, create_args).unwrap();
    let result = module_impl.set_description(
        &id,
        account::SetDescriptionArgs {
            account: account.id,
            description: "New".to_string(),
        },
    );
    assert!(result.is_ok());

    let result = AccountModuleBackend::info(
        &module_impl,
        &id,
        account::InfoArgs {
            account: account.id,
        },
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap().description, Some("New".to_string()));
}

#[test]
/// Verify non-owner is not able to change the description
fn set_description_non_owner() {
    let (mut module_impl, id, create_args) = setup_with_args();
    let account = module_impl.create(&id, create_args).unwrap();
    let result = module_impl.set_description(
        &identity(1),
        account::SetDescriptionArgs {
            account: account.id,
            description: "Other".to_string(),
        },
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code,
        account::errors::user_needs_role("owner").code
    );
}

#[test]
/// Verify we can list account roles
fn list_roles() {
    let (mut module_impl, id, create_args) = setup_with_args();
    let account = module_impl.create(&id, create_args.clone()).unwrap();
    let result = module_impl.list_roles(
        &id,
        account::ListRolesArgs {
            account: account.id,
        },
    );
    assert!(result.is_ok());
    let mut roles = BTreeSet::<account::Role>::new();
    for (_, r) in create_args.roles.unwrap().iter_mut() {
        roles.append(r)
    }
    assert_eq!(result.unwrap().roles, roles,);
}

#[test]
/// Verify we can get given identities account roles
fn get_roles() {
    let (mut module_impl, id, create_args) = setup_with_args();
    let account = module_impl.create(&id, create_args.clone()).unwrap();
    let result = module_impl.get_roles(
        &id,
        account::GetRolesArgs {
            account: account.id,
            identities: VecOrSingle::from(vec![identity(2), identity(3)]),
        },
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap().roles, create_args.roles.unwrap());
}

#[test]
/// Verify we can add new roles
fn add_roles() {
    let (mut module_impl, id, create_args) = setup_with_args();
    let account = module_impl.create(&id, create_args.clone()).unwrap();
    let mut new_role = BTreeMap::from_iter([(
        identity(4),
        BTreeSet::from_iter([account::Role::CanLedgerTransact]),
    )]);
    let result = module_impl.add_roles(
        &id,
        account::AddRolesArgs {
            account: account.id,
            roles: new_role.clone(),
        },
    );
    assert!(result.is_ok());

    let result = module_impl.get_roles(
        &id,
        account::GetRolesArgs {
            account: account.id,
            identities: VecOrSingle::from(vec![identity(2), identity(3), identity(4)]),
        },
    );
    assert!(result.is_ok());
    let mut roles = create_args.roles.unwrap();
    roles.append(&mut new_role);
    assert_eq!(result.unwrap().roles, roles);
}

#[test]
/// Verify non-owner is not able to add role
fn add_roles_non_owner() {
    let (mut module_impl, id, create_args) = setup_with_args();
    let account = module_impl.create(&id, create_args.clone()).unwrap();
    let mut new_role = BTreeMap::from_iter([(
        identity(4),
        BTreeSet::from_iter([account::Role::CanLedgerTransact]),
    )]);
    let mut roles = create_args.roles.unwrap();
    roles.append(&mut new_role);
    let result = module_impl.add_roles(
        &identity(2),
        account::AddRolesArgs {
            account: account.id,
            roles: new_role.clone(),
        },
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code,
        account::errors::user_needs_role("owner").code
    );
}

#[test]
/// Verify we can remove roles
fn remove_roles() {
    let (mut module_impl, id, create_args) = setup_with_args();
    let account = module_impl.create(&id, create_args).unwrap();
    let result = module_impl.remove_roles(
        &id,
        account::RemoveRolesArgs {
            account: account.id,
            roles: BTreeMap::from_iter([(
                identity(2),
                BTreeSet::from_iter([account::Role::CanMultisigApprove]),
            )]),
        },
    );
    assert!(result.is_ok());

    let result = module_impl.get_roles(
        &id,
        account::GetRolesArgs {
            account: account.id,
            identities: VecOrSingle::from(vec![identity(2)]),
        },
    );
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap().roles.get(&identity(2)).unwrap(),
        &BTreeSet::<account::Role>::new()
    );
}

#[test]
// Verify non-owner is not able to remove role
fn remove_roles_non_owner() {
    let (mut module_impl, id, create_args) = setup_with_args();
    let account = module_impl.create(&id, create_args).unwrap();
    let result = module_impl.remove_roles(
        &identity(2),
        account::RemoveRolesArgs {
            account: account.id,
            roles: BTreeMap::from_iter([(
                identity(2),
                BTreeSet::from_iter([account::Role::CanMultisigApprove]),
            )]),
        },
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code,
        account::errors::user_needs_role("owner").code
    );
}

#[test]
/// Verify we can delete account
fn delete() {
    let (mut module_impl, id, create_args) = setup_with_args();
    let account = module_impl.create(&id, create_args).unwrap();
    let result = module_impl.delete(
        &id,
        account::DeleteArgs {
            account: account.id,
        },
    );
    assert!(result.is_ok());

    let result = AccountModuleBackend::info(
        &module_impl,
        &id,
        account::InfoArgs {
            account: account.id,
        },
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code,
        account::errors::unknown_account("").code
    );
}

#[test]
/// Verify non-owner is unable to delete account
fn delete_non_owner() {
    let (mut module_impl, id, create_args) = setup_with_args();
    let account = module_impl.create(&id, create_args).unwrap();
    let result = module_impl.delete(
        &identity(2),
        account::DeleteArgs {
            account: account.id,
        },
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code,
        account::errors::user_needs_role("owner").code
    );
}

/// Verify that add_feature works with a valid feature.
#[test]
fn add_feature() {
    let (mut module_impl, id, create_args) = setup_with_args();
    let account = module_impl.create(&id, create_args).unwrap();

    let info_before = account::AccountModuleBackend::info(
        &module_impl,
        &id,
        account::InfoArgs {
            account: account.id,
        },
    )
    .expect("Could not get info");

    // Prevent test from regressing.
    assert!(!info_before
        .features
        .has_id(account::features::ledger::AccountLedger::ID));

    module_impl
        .add_features(
            &id,
            account::AddFeaturesArgs {
                account: account.id,
                roles: None,
                features: account::features::FeatureSet::from_iter([
                    account::features::ledger::AccountLedger.as_feature(),
                ]),
            },
        )
        .expect("Could not add feature");

    let info_after = account::AccountModuleBackend::info(
        &module_impl,
        &id,
        account::InfoArgs {
            account: account.id,
        },
    )
    .expect("Could not get info");

    assert!(info_after
        .features
        .has_id(account::features::ledger::AccountLedger::ID));
}

/// Verify that add_feature cannot add existing features.
#[test]
fn add_feature_existing() {
    let (mut module_impl, id, create_args) = setup_with_args();
    let account = module_impl.create(&id, create_args).unwrap();

    let info_before = account::AccountModuleBackend::info(
        &module_impl,
        &id,
        account::InfoArgs {
            account: account.id,
        },
    )
    .expect("Could not get info");

    assert!(info_before
        .features
        .has_id(account::features::multisig::MultisigAccountFeature::ID));

    let result = module_impl.add_features(
        &id,
        account::AddFeaturesArgs {
            account: account.id,
            roles: None,
            features: account::features::FeatureSet::from_iter([
                account::features::multisig::MultisigAccountFeature::default().as_feature(),
            ]),
        },
    );
    assert!(result.is_err());

    let info_after = account::AccountModuleBackend::info(
        &module_impl,
        &id,
        account::InfoArgs {
            account: account.id,
        },
    )
    .expect("Could not get info");

    assert!(info_after
        .features
        .has_id(account::features::multisig::MultisigAccountFeature::ID));
}
