use crate::common::{AccountType, Setup, MFX_SYMBOL};
use many_identity::testing::identity;
use many_identity::Address;
use many_ledger::migration::memo::MEMO_MIGRATION;
use many_modules::account::features::multisig::AccountMultisigModuleBackend;
use many_modules::events::{EventInfo, EventsModuleBackend, ListArgs};
use many_modules::{account, events, ledger};
use many_types::ledger::TokenAmount;
use many_types::memo::MemoLegacy;
use many_types::{Either, Memo};

#[test]
fn memo_migration_works() {
    fn make_multisig_transaction(h: &mut Setup, account_id: Address) {
        let send_tx = events::AccountMultisigTransaction::Send(ledger::SendArgs {
            from: Some(account_id),
            to: identity(10),
            symbol: *MFX_SYMBOL,
            amount: TokenAmount::from(10_000u16),
        });

        let tx = account::features::multisig::SubmitTransactionArgs {
            account: account_id,
            memo_: Some(MemoLegacy::try_from("Legacy Memo".to_string()).unwrap()),
            transaction: Box::new(send_tx),
            threshold: None,
            timeout_in_secs: None,
            execute_automatically: None,
            data_: Some(b"Legacy Data".to_vec().try_into().unwrap()),
            // This should be ignored as it would be backward incompatible
            // before the migration is active.
            memo: Some(Memo::try_from("Memo").unwrap()),
        };
        h.module_impl
            .multisig_submit_transaction(&identity(1), tx)
            .map(|x| x.token)
            .unwrap();
    }

    fn check_events(
        harness: &Setup,
        expected_nb_events: u64,
        assert_fn: fn((usize, events::EventLog)),
    ) {
        let events = harness.module_impl.list(ListArgs::default()).unwrap();

        assert_eq!(events.nb_events, expected_nb_events);
        events.events.into_iter().enumerate().for_each(assert_fn);
    }

    // Setup starts with 2 accounts because of staging/ledger_state.json5
    let mut harness = Setup::new_with_migrations(true, [(5, &MEMO_MIGRATION)]);
    harness.set_balance(harness.id, 1_000_000, *MFX_SYMBOL);
    let (_, account_id) = harness.block(|h| {
        // Create an account.
        h.create_account_as_(identity(1), AccountType::Multisig)
    });

    harness.block(|h| make_multisig_transaction(h, account_id));

    check_events(&harness, 2, |(_, ev)| {
        if let EventInfo::AccountMultisigSubmit {
            memo_, data_, memo, ..
        } = ev.content
        {
            assert_eq!(memo_.as_ref().map(|x| x.as_ref()), Some("Legacy Memo"));
            assert_eq!(
                data_.as_ref().map(|x| x.as_bytes()),
                Some(b"Legacy Data".as_slice())
            );
            assert!(memo.is_none());
        }
    });

    // Wait 2 block for the migration to run.
    for _ in 0..2 {
        harness.block(|_| {});
        check_events(&harness, 2, |(_, ev)| {
            if let EventInfo::AccountMultisigSubmit {
                memo_, data_, memo, ..
            } = ev.content
            {
                assert_eq!(memo_.as_ref().map(|x| x.as_ref()), Some("Legacy Memo"));
                assert_eq!(
                    data_.as_ref().map(|x| x.as_bytes()),
                    Some(b"Legacy Data".as_slice())
                );
                assert!(memo.is_none());
            }
        });
    }

    // Wait 1 more block, migration activates here.
    harness.block(|_| {});

    check_events(&harness, 2, |(_, ev)| {
        if let EventInfo::AccountMultisigSubmit {
            memo_, data_, memo, ..
        } = ev.content
        {
            assert!(memo_.is_none());
            assert!(data_.is_none());
            assert_eq!(
                memo.unwrap(),
                Memo::try_from_iter([
                    Either::Left("Legacy Memo".to_string()),
                    Either::Right(b"Legacy Data".to_vec())
                ])
                .unwrap()
            );
        }
    });

    // Add a new event after migration is active.
    harness.block(|h| make_multisig_transaction(h, account_id));

    check_events(&harness, 3, |(idx, ev)| {
        if let EventInfo::AccountMultisigSubmit {
            memo_, data_, memo, ..
        } = ev.content
        {
            assert!(memo_.is_none());
            assert!(data_.is_none());

            let memo = memo.unwrap();
            assert!(
                (memo == "Memo")
                    || memo
                        == Memo::try_from_iter([
                            Either::Left("Legacy Memo".to_string()),
                            Either::Right(b"Legacy Data".to_vec())
                        ])
                        .unwrap(),
                "Memo does not match. {idx}: {memo:?}"
            );
        }
    });
}
