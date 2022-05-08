use many::server::module::abci_backend::AbciLoadSnapshotChunk;
use many::types::ledger::TokenAmount;
use many::Identity;
use many_ledger::storage::SNAPSHOT_INTERVAL;
use std::collections::BTreeMap;

#[test]
fn drop_used_snapshot() {
    let persistent_path = tempfile::tempdir().unwrap();
    let snapshot_path = tempfile::tempdir().unwrap();

    println!("snapshot test dir: {}", snapshot_path.as_ref().display());
    let symbol0 = Identity::anonymous();
    let id0 = Identity::public_key_raw([0; 28]);
    let symbols = BTreeMap::from_iter(vec![(symbol0, "FBT".to_string())].into_iter());
    let balances = BTreeMap::from([(id0, BTreeMap::from([(symbol0, TokenAmount::from(1000u16))]))]);

    let mut storage = many_ledger::storage::LedgerStorage::new(
        symbols,
        balances,
        persistent_path,
        snapshot_path,
        false,
    )
    .unwrap();

    storage.commit();

    let request_chunk = |height, chunk| {
        let req = AbciLoadSnapshotChunk {
            height,
            format: 0,
            chunk,
        };
        println!("snapshot Chunk and Height {:?}, {}", chunk, height);
        req
    };

    storage
        .load_snapshot_chunk(request_chunk(SNAPSHOT_INTERVAL, 0))
        .unwrap();

    let snap = storage.list_snapshots();

    assert_eq!(storage.hash(), snap.snapshots[0].hash);
    println!(
        "snapshot Hash - Hash {:?}, {:?}",
        storage.hash(),
        snap.snapshots[0].hash
    );

    let snap_one = storage.create_snapshot(SNAPSHOT_INTERVAL).unwrap();
    let get_snap = storage.list_snapshots();

    println!(
        "snapshot Meta - Meta {:?}, {:?}",
        snap_one.metadata, get_snap.snapshots[0].metadata
    );
    assert_eq!(snap_one.metadata, get_snap.snapshots[0].metadata);

    let snap2 = storage.create_snapshot(SNAPSHOT_INTERVAL + 1).unwrap();
    let list2 = storage.list_snapshots();

    assert_eq!(snap2.height, list2.snapshots[0].height);
    assert_eq!(snap2.hash, list2.snapshots[0].hash);
    assert_eq!(snap2.metadata, list2.snapshots[0].metadata);
}
