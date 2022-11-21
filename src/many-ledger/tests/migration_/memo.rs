use crate::common::{AccountType, Setup, MFX_SYMBOL};
use many_identity::testing::identity;
use many_ledger::migration::memo::MEMO_MIGRATION;
use many_modules::account::features::multisig::AccountMultisigModuleBackend;
use many_modules::events::{EventInfo, EventsModuleBackend, ListArgs};
use many_modules::{account, events, ledger};
use many_types::ledger::TokenAmount;
use many_types::memo::MemoLegacy;

#[test]
fn memo_migration_works() {
    // Setup starts with 2 accounts because of staging/ledger_state.json5
    let mut harness = Setup::new_with_migrations(true, [(3, &MEMO_MIGRATION, false)]);
    harness.set_balance(harness.id, 1_000_000, *MFX_SYMBOL);
    let (_, account_id) = harness.block(|h| {
        // Create an account.
        h.create_account_as_(identity(1), AccountType::Multisig)
    });

    let (_height, _) = harness.block(|h| {
        let send_tx = events::AccountMultisigTransaction::Send(ledger::SendArgs {
            from: Some(account_id),
            to: identity(10),
            symbol: *MFX_SYMBOL,
            amount: TokenAmount::from(10_000u16),
        });

        let tx = account::features::multisig::SubmitTransactionArgs {
            account: account_id,
            memo_: Some(MemoLegacy::try_from("Foo".to_string()).unwrap()),
            transaction: Box::new(send_tx),
            threshold: None,
            timeout_in_secs: None,
            execute_automatically: None,
            data_: Some(b"Bar".to_vec().try_into().unwrap()),
            memo: None,
        };
        h.module_impl
            .multisig_submit_transaction(&identity(1), tx)
            .map(|x| x.token)
            .unwrap();
    });

    // List events.
    let events = harness
        .module_impl
        .list(ListArgs {
            count: Some(10),
            order: None,
            filter: None,
        })
        .unwrap();

    assert_eq!(events.nb_events, 2);
    assert!(matches!(
        events.events.get(1).unwrap().content,
        EventInfo::AccountMultisigSubmit {
            memo_: Some(_),
            data_: Some(_),
            memo: None,
            ..
        },
    ));

    // Wait 1 block for the migration to run.
    harness.block(|_| {});

    let events = harness
        .module_impl
        .list(ListArgs {
            count: Some(10),
            order: None,
            filter: None,
        })
        .unwrap();

    // List events again.
    assert_eq!(events.nb_events, 2);

    assert!(matches!(
        events.events.last().unwrap().content,
        EventInfo::AccountMultisigSubmit {
            memo_: None,
            data_: None,
            memo: Some(_),
            ..
        },
    ));
}
