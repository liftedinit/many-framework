use many_identity::testing::identity;
use many_identity::Address;
use many_ledger::{module::LedgerModuleImpl, storage::LedgerStorage};
use many_modules::account::features::FeatureInfo;
use many_modules::account::AccountModuleBackend;
use many_modules::ledger::LedgerModuleBackend;
use many_modules::{account, ledger};
use std::collections::{BTreeMap, BTreeSet};

/// Verify persistent storage can be re-loaded
#[test]
fn load() {
    let path = tempfile::tempdir().unwrap().into_path();
    #[allow(unused_assignments)]
    let mut id = Address::anonymous();
    // Storage needs to become out-of-scope so it can be re-opened
    {
        let _ = LedgerStorage::new(
            BTreeMap::from([(identity(1000), "MF0".to_string())]),
            BTreeMap::from([(
                identity(5),
                BTreeMap::from([(identity(1000), 10000000u64.into())]),
            )]),
            path.clone(),
            identity(666),
            false,
            None,
            None,
        );
        let mut module_impl = LedgerModuleImpl::load(None, path.clone(), false).unwrap();

        id = module_impl
            .create(
                &identity(3),
                account::CreateArgs {
                    description: None,
                    roles: Some(BTreeMap::from([(
                        identity(1),
                        BTreeSet::from([account::Role::Owner]),
                    )])),
                    features: account::features::FeatureSet::from_iter([
                        account::features::ledger::AccountLedger.as_feature(),
                    ]),
                },
            )
            .unwrap()
            .id;
    }

    let module_impl = LedgerModuleImpl::load(None, path, false).unwrap();
    let balance = module_impl
        .balance(
            &identity(5),
            ledger::BalanceArgs {
                account: Some(identity(5)),
                symbols: Some(vec![identity(1000)].into()),
            },
        )
        .unwrap();
    assert_eq!(
        balance.balances,
        BTreeMap::from([(identity(1000), 10000000u64.into())])
    );

    let role = module_impl
        .get_roles(
            &identity(3),
            account::GetRolesArgs {
                account: id,
                identities: vec![identity(1)].into(),
            },
        )
        .unwrap()
        .roles;
    assert_eq!(
        role,
        BTreeMap::from([(identity(1), BTreeSet::from([account::Role::Owner]),)])
    );
}
