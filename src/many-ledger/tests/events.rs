pub mod common;
use std::ops::Bound;

use common::*;
use many::server::module::ledger::{LedgerCommandsModuleBackend, LedgerTransactionsModuleBackend};
use many::server::module::{self};
use many::types::identity::testing::identity;
use many::types::ledger::{TransactionInfo, TransactionKind};
use many::types::{CborRange, Timestamp, TransactionFilter};
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
fn transactions() {
    let Setup {
        mut module_impl,
        id,
        ..
    } = setup();
    let result = module_impl.transactions(module::ledger::TransactionsArgs {});
    assert!(result.is_ok());
    assert_eq!(result.unwrap().nb_transactions, 0);
    send(&mut module_impl, id, identity(1));
    let result = module_impl.transactions(module::ledger::TransactionsArgs {});
    assert!(result.is_ok());
    assert_eq!(result.unwrap().nb_transactions, 1);
}

#[test]
fn list() {
    let Setup {
        mut module_impl,
        id,
        ..
    } = setup();
    send(&mut module_impl, id, identity(1));
    let result = module_impl.list(module::ledger::ListArgs {
        count: None,
        order: None,
        filter: None,
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_transactions, 1);
    assert_eq!(list_return.transactions.len(), 1);
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
    let result = module_impl.list(module::ledger::ListArgs {
        count: None,
        order: None,
        filter: Some(TransactionFilter {
            account: Some(vec![account_id].into()),
            ..TransactionFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_transactions, 3);
    assert_eq!(list_return.transactions.len(), 2); // 1 send + 1 create
    for tx in list_return.transactions {
        match tx.content {
            TransactionInfo::AccountCreate { account, .. } => {
                assert_eq!(account, account_id);
            }
            TransactionInfo::Send { from, .. } => {
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
    let result = module_impl.list(module::ledger::ListArgs {
        count: None,
        order: None,
        filter: Some(TransactionFilter {
            kind: Some(vec![TransactionKind::Send].into()),
            ..TransactionFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_transactions, 1);
    assert_eq!(list_return.transactions.len(), 1);
    assert_eq!(list_return.transactions[0].kind(), TransactionKind::Send);
    assert_eq!(list_return.transactions[0].symbol(), Some(&*MFX_SYMBOL));
    assert!(list_return.transactions[0].is_about(&id));
}

#[test]
fn list_filter_symbol() {
    let Setup {
        mut module_impl,
        id,
        ..
    } = setup();
    send(&mut module_impl, id, identity(1));
    let result = module_impl.list(module::ledger::ListArgs {
        count: None,
        order: None,
        filter: Some(TransactionFilter {
            symbol: Some(vec![*MFX_SYMBOL].into()),
            ..TransactionFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_transactions, 1);
    assert_eq!(list_return.transactions.len(), 1);
    assert_eq!(list_return.transactions[0].kind(), TransactionKind::Send);
    assert_eq!(list_return.transactions[0].symbol(), Some(&*MFX_SYMBOL));
    assert!(list_return.transactions[0].is_about(&id));

    let result = module_impl.list(module::ledger::ListArgs {
        count: None,
        order: None,
        filter: Some(TransactionFilter {
            symbol: Some(vec![identity(100)].into()),
            ..TransactionFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.transactions.len(), 0);
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
    let result = module_impl.list(module::ledger::ListArgs {
        count: None,
        order: None,
        filter: Some(TransactionFilter {
            date_range: Some(CborRange {
                start: Bound::Included(before),
                end: Bound::Included(after),
            }),
            ..TransactionFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_transactions, 1);
    assert_eq!(list_return.transactions.len(), 1);
    assert_eq!(list_return.transactions[0].kind(), TransactionKind::Send);
    assert_eq!(list_return.transactions[0].symbol(), Some(&*MFX_SYMBOL));
    assert!(list_return.transactions[0].is_about(&id));

    // TODO: Remove this when we support factional seconds
    // See https://github.com/liftedinit/many-rs/issues/110
    std::thread::sleep(std::time::Duration::new(1, 0));
    let now = Timestamp::now();
    let result = module_impl.list(module::ledger::ListArgs {
        count: None,
        order: None,
        filter: Some(TransactionFilter {
            date_range: Some(CborRange {
                start: Bound::Included(now),
                end: Bound::Unbounded,
            }),
            ..TransactionFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.transactions.len(), 0);
}
