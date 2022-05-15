use many::server::module::account::features::multisig::SubmitTransactionArg;
use many::server::module::account::features::{multisig, FeatureInfo, FeatureSet};
use many::server::module::account::{Account, CreateArgs};
use many::types::ledger::{Symbol, TokenAmount};
use many::Identity;
use many_ledger::storage::LedgerStorage;
use std::collections::{BTreeMap, BTreeSet};

fn identity(key: u8) -> Identity {
    Identity::public_key_raw([key; 28])
}

fn symbol(key: u8) -> Identity {
    Identity::public_key_raw([key; 28]).with_subresource_id_unchecked(1)
}

fn setup<B: IntoIterator<Item = (Identity, u64)>>(b: B) -> LedgerStorage {
    let symbols = BTreeMap::from_iter([
        (symbol(0), "X0".to_string()),
        (symbol(1), "X1".to_string()),
        (symbol(2), "X2".to_string()),
    ]);

    let mut balances: BTreeMap<Identity, BTreeMap<Symbol, TokenAmount>> = BTreeMap::new();
    for (key, value) in b {
        balances
            .entry(key)
            .or_default()
            .insert(symbol(0), TokenAmount::from(value));
    }

    let persistent_path = tempfile::tempdir().unwrap();

    let storage = many_ledger::storage::LedgerStorage::new(
        symbols,
        balances,
        persistent_path,
        identity(0),
        false,
    )
    .unwrap();

    storage
}

fn create_account(storage: &mut LedgerStorage, account_owner: &Identity) -> Identity {
    storage
        .add_account(Account::create(
            &account_owner,
            CreateArgs {
                description: None,
                roles: Some(BTreeMap::from_iter([
                    (
                        identity(2),
                        BTreeSet::from_iter(["canMultisigApprove".to_string()]),
                    ),
                    (
                        identity(3),
                        BTreeSet::from_iter(["canMultisigSubmit".to_string()]),
                    ),
                ])),
                features: FeatureSet::from_iter([
                    multisig::MultisigAccountFeature::default().as_feature()
                ]),
            },
        ))
        .expect("Could not create an account")
}

#[test]
fn basic() {
    let mut storage = setup([(identity(1), 10000000)]);

    let account_owner = identity(1);
    let account_id = create_account(&mut storage, &account_owner);

    storage
        .send(&identity(1), &account_id, &symbol(0), 1000000u32.into())
        .expect("Could not send");

    let token = storage
        .create_multisig_transaction(
            &account_owner,
            SubmitTransactionArg::send(account_id, identity(4), symbol(0), 1000u32.into()),
        )
        .expect("Could not create multisig transaction");

    // Try to execute, expect error, should need 3 signatures.
    assert!(storage.execute_multisig(&account_owner, &token).is_err());
    assert_eq!(
        storage
            .get_multisig_info(&token)
            .map(|i| i.info.approvers.values().cloned().collect()),
        Ok(vec![
            multisig::ApproverInfo { approved: true },
            multisig::ApproverInfo { approved: false },
            multisig::ApproverInfo { approved: false }
        ])
    );

    storage
        .approve_multisig(&identity(3), &token)
        .expect("Could not approve.");
    assert!(storage.execute_multisig(&account_owner, &token).is_err());
    assert_eq!(
        storage
            .get_multisig_info(&token)
            .map(|i| i.info.approvers.values().cloned().collect()),
        Ok(vec![
            multisig::ApproverInfo { approved: true },
            multisig::ApproverInfo { approved: false },
            multisig::ApproverInfo { approved: true }
        ])
    );

    storage
        .approve_multisig(&identity(2), &token)
        .expect("Could not approve.");
    // All approvals now.

    // Cannot execute from a non-owner.
    assert!(storage.execute_multisig(&identity(4), &token).is_err());
    // Cannot execute from the non-submitter.
    assert!(storage.execute_multisig(&identity(2), &token).is_err());

    assert!(storage.execute_multisig(&account_owner, &token).is_ok());

    assert_eq!(storage.get_balance(&account_id, &symbol(0)), 999000u32);
    assert_eq!(storage.get_balance(&identity(4), &symbol(0)), 1000u32);
}

#[test]
fn automatic() {
    let mut storage = setup([(identity(1), 10000000)]);

    let account_owner = identity(1);
    let account_id = create_account(&mut storage, &account_owner);

    storage
        .send(&identity(1), &account_id, &symbol(0), 1000000u32.into())
        .expect("Could not send");

    let mut tx_arg = SubmitTransactionArg::send(account_id, identity(4), symbol(0), 1000u32.into());
    tx_arg.execute_automatically = Some(true);

    let token = storage
        .create_multisig_transaction(&account_owner, tx_arg)
        .expect("Could not create multisig transaction");

    // Try to execute, expect error, should need 3 signatures.
    assert!(storage.execute_multisig(&account_owner, &token).is_err());

    // Approve once, try to execute, expect error, should need 3 signatures.
    storage
        .approve_multisig(&identity(3), &token)
        .expect("Could not approve.");
    assert!(storage.execute_multisig(&account_owner, &token).is_err());

    // All the approvals, should have executed without calling execute.
    storage
        .approve_multisig(&identity(2), &token)
        .expect("Could not approve.");
    assert_eq!(storage.get_balance(&account_id, &symbol(0)), 999000u32);
    assert_eq!(storage.get_balance(&identity(4), &symbol(0)), 1000u32);

    // Calling execute on an executed transaction should error.
    assert!(storage.execute_multisig(&account_owner, &token).is_err());
}

#[test]
fn withdraw() {
    let mut storage = setup([(identity(1), 10000000)]);

    let account_owner = identity(1);
    let account_id = create_account(&mut storage, &account_owner);

    storage
        .send(&identity(1), &account_id, &symbol(0), 1000000u32.into())
        .expect("Could not send");

    let mut tx_arg = SubmitTransactionArg::send(account_id, identity(4), symbol(0), 1000u32.into());
    tx_arg.execute_automatically = Some(true);

    let token = storage
        .create_multisig_transaction(&account_owner, tx_arg)
        .expect("Could not create multisig transaction");

    // Try to execute, expect error, should need 3 signatures.
    assert!(storage.execute_multisig(&account_owner, &token).is_err());

    // Approve once, try to execute, expect error, should need 3 signatures.
    storage
        .approve_multisig(&identity(3), &token)
        .expect("Could not approve.");
    assert!(storage.execute_multisig(&account_owner, &token).is_err());

    // Shouldn't be able to withdraw from a non-owner account.
    assert!(storage.withdraw_multisig(&identity(2), &token).is_err());
    assert!(storage.withdraw_multisig(&account_owner, &token).is_ok());

    // This is as if the transaction never existed.
    assert!(storage.approve_multisig(&identity(2), &token).is_err());
    assert!(storage.revoke_multisig(&identity(2), &token).is_err());
    assert!(storage.withdraw_multisig(&account_owner, &token).is_err());
    assert!(storage.execute_multisig(&account_owner, &token).is_err());

    // No balance should be changed.
    assert_eq!(storage.get_balance(&account_id, &symbol(0)), 1000000u32);
    assert_eq!(storage.get_balance(&identity(4), &symbol(0)), 0u32);
}
