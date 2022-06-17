pub mod common;
use std::ops::Bound;

use common::*;
use many::server::module::events::EventsModuleBackend;
use many::server::module::ledger::LedgerCommandsModuleBackend;
use many::server::module::{self};
use many::types::events::{EventFilter, EventInfo, EventKind};
use many::types::identity::testing::identity;
use many::types::{CborRange, Timestamp};
use many_ledger::module::LedgerModuleImpl;

fn send(module_impl: &mut LedgerModuleImpl, from: many::Identity, to: many::Identity) {
    module_impl.set_balance_only_for_testing(from, 1000, *MFX_SYMBOL);
    let result = module_impl.send(
        &from,
        module::ledger::SendArgs {
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
    let result = EventsModuleBackend::info(&module_impl, module::events::InfoArgs {});
    assert!(result.is_ok());
    assert_eq!(result.unwrap().total, 0);
    send(&mut module_impl, id, identity(1));
    let result = EventsModuleBackend::info(&module_impl, module::events::InfoArgs {});
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
    let result = module_impl.list(module::events::ListArgs {
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
    let result = module_impl.list(module::events::ListArgs {
        count: None,
        order: None,
        filter: Some(EventFilter {
            account: Some(vec![account_id].into()),
            ..EventFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_events, 3);
    assert_eq!(list_return.events.len(), 2); // 1 send + 1 create
    for event in list_return.events {
        match event.content {
            EventInfo::AccountCreate { account, .. } => {
                assert_eq!(account, account_id);
            }
            EventInfo::Send { from, .. } => {
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
    let result = module_impl.list(module::events::ListArgs {
        count: None,
        order: None,
        filter: Some(EventFilter {
            kind: Some(vec![EventKind::Send].into()),
            ..EventFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_events, 1);
    assert_eq!(list_return.events.len(), 1);
    assert_eq!(list_return.events[0].kind(), EventKind::Send);
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
    let result = module_impl.list(module::events::ListArgs {
        count: None,
        order: None,
        filter: Some(EventFilter {
            symbol: Some(vec![*MFX_SYMBOL].into()),
            ..EventFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_events, 1);
    assert_eq!(list_return.events.len(), 1);
    assert_eq!(list_return.events[0].kind(), EventKind::Send);
    assert_eq!(list_return.events[0].symbol(), Some(&*MFX_SYMBOL));
    assert!(list_return.events[0].is_about(&id));

    let result = module_impl.list(module::events::ListArgs {
        count: None,
        order: None,
        filter: Some(EventFilter {
            symbol: Some(vec![identity(100)].into()),
            ..EventFilter::default()
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
    let result = module_impl.list(module::events::ListArgs {
        count: None,
        order: None,
        filter: Some(EventFilter {
            date_range: Some(CborRange {
                start: Bound::Included(before),
                end: Bound::Included(after),
            }),
            ..EventFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_events, 1);
    assert_eq!(list_return.events.len(), 1);
    assert_eq!(list_return.events[0].kind(), EventKind::Send);
    assert_eq!(list_return.events[0].symbol(), Some(&*MFX_SYMBOL));
    assert!(list_return.events[0].is_about(&id));

    // TODO: Remove this when we support factional seconds
    // See https://github.com/liftedinit/many-rs/issues/110
    std::thread::sleep(std::time::Duration::new(1, 0));
    let now = Timestamp::now();
    let result = module_impl.list(module::events::ListArgs {
        count: None,
        order: None,
        filter: Some(EventFilter {
            date_range: Some(CborRange {
                start: Bound::Included(now),
                end: Bound::Unbounded,
            }),
            ..EventFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.events.len(), 0);
}
