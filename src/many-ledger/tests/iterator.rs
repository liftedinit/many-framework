use many::types::ledger::{TokenAmount, Transaction, TransactionId};
use many::types::{CborRange, SortOrder};
use many::Identity;
use many_ledger::storage::LedgerStorage;
use std::collections::BTreeMap;
use std::ops::Bound;

fn setup() -> LedgerStorage {
    let symbol0 = Identity::anonymous();
    let id0 = Identity::public_key_raw([0; 28]);
    let id1 = Identity::public_key_raw([1; 28]);

    let symbols = BTreeMap::from_iter(vec![(symbol0, "FBT".to_string())].into_iter());
    let balances = BTreeMap::new();
    let persistent_path = tempfile::tempdir().unwrap();
    let snapshot_path = tempfile::tempdir().unwrap();

    let mut storage = many_ledger::storage::LedgerStorage::new(
        symbols,
        balances,
        BTreeMap::new(),
        persistent_path,
        snapshot_path,
        false,
    )
    .unwrap();

    //  storage
    //      .mint(&id0, &symbol0, TokenAmount::from(1000u16))
    //      .unwrap();
    for _ in 0..5 {
        storage
            .send(&id0, &id1, &symbol0, TokenAmount::from(100u16))
            .unwrap();
    }

    // Check that we have 6 transactions (5 sends and a mint).
    assert_eq!(storage.nb_transactions(), 6);

    storage
}

fn iter_asc(
    storage: &LedgerStorage,
    start: Bound<TransactionId>,
    end: Bound<TransactionId>,
) -> impl Iterator<Item = Transaction> + '_ {
    storage
        .iter(CborRange { start, end }, SortOrder::Ascending)
        .into_iter()
        .map(|(_, v)| minicbor::decode(&v).expect("Iterator item not a transaction."))
}

#[test]
fn range_works() {
    let storage = setup();

    // Get the first transaction ID.
    let mut iter = iter_asc(&storage, Bound::Unbounded, Bound::Unbounded);
    let first_tx = iter.next().expect("No transactions?");
    let first_id = first_tx.id;
    let last_tx = iter.last().expect("Only 1 transaction");
    let last_id = last_tx.id;

    // Make sure exclusive range removes the first_id.
    assert!(iter_asc(
        &storage,
        Bound::Excluded(first_id.clone()),
        Bound::Unbounded
    )
    .all(|x| x.id != first_id));

    let iter = iter_asc(
        &storage,
        Bound::Excluded(first_id.clone()),
        Bound::Unbounded,
    );
    assert_eq!(iter.last().expect("Should have a last item").id, last_id);

    // Make sure exclusive range removes the last_id.
    assert!(
        iter_asc(&storage, Bound::Unbounded, Bound::Excluded(last_id.clone()))
            .all(|x| x.id != last_id)
    );

    let mut iter = iter_asc(&storage, Bound::Unbounded, Bound::Excluded(last_id.clone()));
    assert_eq!(iter.next().expect("Should have a first item").id, first_id);

    // Make sure inclusive bounds include first_id.
    let mut iter = iter_asc(
        &storage,
        Bound::Included(first_id.clone()),
        Bound::Included(last_id.clone()),
    );
    assert_eq!(iter.next().expect("Should have a first item").id, first_id);
    assert_eq!(iter.last().expect("Should have a last item").id, last_id);
}
