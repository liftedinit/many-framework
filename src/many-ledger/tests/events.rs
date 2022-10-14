pub mod common;

use common::*;
use many_identity::testing::identity;
use many_identity::Address;
use many_ledger::module::LedgerModuleImpl;
use many_modules::account::features::multisig::{
    self, AccountMultisigModuleBackend, Memo, MultisigTransactionState,
};
use many_modules::events::{
    self, EventFilterAttributeSpecific, EventFilterAttributeSpecificIndex, EventsModuleBackend,
};
use many_modules::ledger;
use many_modules::ledger::LedgerCommandsModuleBackend;
use many_types::{CborRange, Timestamp};
use proptest::prelude::*;
use proptest::test_runner::Config;
use std::collections::BTreeMap;
use std::ops::Bound;

fn send(module_impl: &mut LedgerModuleImpl, from: Address, to: Address) {
    module_impl.set_balance_only_for_testing(from, 1000, *MFX_SYMBOL);
    let result = module_impl.send(
        &from,
        ledger::SendArgs {
            from: Some(from),
            to,
            amount: 10u16.into(),
            symbol: *MFX_SYMBOL,
        },
    );
    assert!(result.is_ok());
}

#[test]
fn events() {
    let Setup {
        mut module_impl,
        id,
        ..
    } = setup();
    let result = events::EventsModuleBackend::info(&module_impl, events::InfoArgs {});
    assert!(result.is_ok());
    assert_eq!(result.unwrap().total, 0);
    send(&mut module_impl, id, identity(1));
    let result = events::EventsModuleBackend::info(&module_impl, events::InfoArgs {});
    assert!(result.is_ok());
    assert_eq!(result.unwrap().total, 1);
}

#[test]
fn list() {
    let Setup {
        mut module_impl,
        id,
        ..
    } = setup();
    send(&mut module_impl, id, identity(1));
    let result = module_impl.list(events::ListArgs {
        count: None,
        order: None,
        filter: None,
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_events, 1);
    assert_eq!(list_return.events.len(), 1);
}

#[test]
fn list_filter_account() {
    let SetupWithAccount {
        mut module_impl,
        account_id,
        id,
    } = setup_with_account(AccountType::Ledger);
    send(&mut module_impl, id, identity(3));
    send(&mut module_impl, account_id, identity(1));
    let result = module_impl.list(events::ListArgs {
        count: None,
        order: None,
        filter: Some(events::EventFilter {
            account: Some(vec![account_id].into()),
            ..events::EventFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_events, 3);
    assert_eq!(list_return.events.len(), 2); // 1 send + 1 create
    for event in list_return.events {
        match event.content {
            events::EventInfo::AccountCreate { account, .. } => {
                assert_eq!(account, account_id);
            }
            events::EventInfo::Send { from, .. } => {
                assert_eq!(from, account_id);
            }
            _ => unimplemented!(),
        }
    }
}

#[test]
fn list_filter_kind() {
    let Setup {
        mut module_impl,
        id,
        ..
    } = setup();
    send(&mut module_impl, id, identity(1));
    let result = module_impl.list(events::ListArgs {
        count: None,
        order: None,
        filter: Some(events::EventFilter {
            kind: Some(vec![events::EventKind::Send].into()),
            ..events::EventFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_events, 1);
    assert_eq!(list_return.events.len(), 1);
    assert_eq!(list_return.events[0].kind(), events::EventKind::Send);
    assert_eq!(list_return.events[0].symbol(), Some(&*MFX_SYMBOL));
    assert!(list_return.events[0].is_about(&id));
}

#[test]
fn list_filter_symbol() {
    let Setup {
        mut module_impl,
        id,
        ..
    } = setup();
    send(&mut module_impl, id, identity(1));
    let result = module_impl.list(events::ListArgs {
        count: None,
        order: None,
        filter: Some(events::EventFilter {
            symbol: Some(vec![*MFX_SYMBOL].into()),
            ..events::EventFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_events, 1);
    assert_eq!(list_return.events.len(), 1);
    assert_eq!(list_return.events[0].kind(), events::EventKind::Send);
    assert_eq!(list_return.events[0].symbol(), Some(&*MFX_SYMBOL));
    assert!(list_return.events[0].is_about(&id));

    let result = module_impl.list(events::ListArgs {
        count: None,
        order: None,
        filter: Some(events::EventFilter {
            symbol: Some(vec![identity(100)].into()),
            ..events::EventFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.events.len(), 0);
}

#[test]
fn list_filter_date() {
    let Setup {
        mut module_impl,
        id,
        ..
    } = setup();
    let before = Timestamp::now();
    send(&mut module_impl, id, identity(1));
    // TODO: Remove this when we support factional seconds
    // See https://github.com/liftedinit/many-rs/issues/110
    std::thread::sleep(std::time::Duration::new(1, 0));
    let after = Timestamp::now();
    let result = module_impl.list(events::ListArgs {
        count: None,
        order: None,
        filter: Some(events::EventFilter {
            date_range: Some(CborRange {
                start: Bound::Included(before),
                end: Bound::Included(after),
            }),
            ..events::EventFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_events, 1);
    assert_eq!(list_return.events.len(), 1);
    assert_eq!(list_return.events[0].kind(), events::EventKind::Send);
    assert_eq!(list_return.events[0].symbol(), Some(&*MFX_SYMBOL));
    assert!(list_return.events[0].is_about(&id));

    // TODO: Remove this when we support factional seconds
    // See https://github.com/liftedinit/many-rs/issues/110
    std::thread::sleep(std::time::Duration::new(1, 0));
    let now = Timestamp::now();
    let result = module_impl.list(events::ListArgs {
        count: None,
        order: None,
        filter: Some(events::EventFilter {
            date_range: Some(CborRange {
                start: Bound::Included(now),
                end: Bound::Unbounded,
            }),
            ..events::EventFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.events.len(), 0);
}

fn submit_args(
    account_id: Address,
    transaction: events::AccountMultisigTransaction,
    execute_automatically: Option<bool>,
) -> multisig::SubmitTransactionArgs {
    multisig::SubmitTransactionArgs {
        account: account_id,
        memo: Some(Memo::try_from("Foo".to_string()).unwrap()),
        transaction: Box::new(transaction),
        threshold: None,
        timeout_in_secs: None,
        execute_automatically,
        data: None,
    }
}

proptest! {
    #![proptest_config(Config {cases: 200, source_file: Some("tests/events"), .. Config::default()})]

    #[test]
    fn list_filter_attribute_specific(SetupWithAccountAndTx {
        mut module_impl,
        id,
        account_id,
        tx,
    } in setup_with_account_and_tx(AccountType::Multisig)) {
        let submit_args = submit_args(account_id, tx.clone(), None);
        module_impl
            .multisig_submit_transaction(&id, submit_args.clone())
            .expect("Multisig transaction should be sent");

        let result = module_impl.list(events::ListArgs {
            count: None,
            order: None,
            filter: Some(events::EventFilter{
                events_filter_attribute_specific: BTreeMap::from([
                    (EventFilterAttributeSpecificIndex::MultisigTransactionState,
                     EventFilterAttributeSpecific::MultisigTransactionState(vec![MultisigTransactionState::Pending].into()))
                ]),
                ..events::EventFilter::default()
            })
        }).expect("List should return a value");

        assert!(!result.events.is_empty());

        let result = module_impl.list(events::ListArgs {
            count: None,
            order: None,
            filter: Some(events::EventFilter{
                events_filter_attribute_specific: BTreeMap::from([
                    (EventFilterAttributeSpecificIndex::MultisigTransactionState,
                     EventFilterAttributeSpecific::MultisigTransactionState(vec![MultisigTransactionState::Withdrawn].into()))
                ]),
                ..events::EventFilter::default()
            })
        }).expect("List should return a value");
        assert!(result.events.is_empty());
    }
}
