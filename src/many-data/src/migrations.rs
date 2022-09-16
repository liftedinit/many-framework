use many_types::ledger::TokenAmount;
use merk::rocksdb::{self, ReadOptions};
use merk::Op;

pub fn initial_metrics_data(persistent_store: &merk::Merk) -> Vec<(Vec<u8>, Op)> {
    let mut total_accounts: u64 = 0;
    let mut non_zero: u64 = 0;

    let mut upper_bound = b"/balances".to_vec();
    *upper_bound.last_mut().unwrap() += 1;
    let mut opts = ReadOptions::default();
    opts.set_iterate_upper_bound(upper_bound);

    let iterator = persistent_store.iter_opt(
        rocksdb::IteratorMode::From(b"/balances", rocksdb::Direction::Forward),
        opts,
    );
    for item in iterator {
        let (_, value) = item.expect("Error while reading the DB");
        total_accounts += 1;
        if TokenAmount::from(value.to_vec()) != 0u16 {
            non_zero += 1
        }
    }
    vec![
        (
            b"/data/account_total_count".to_vec(),
            Op::Put(total_accounts.to_be_bytes().to_vec()),
        ),
        (
            b"/data/non_zero_account_total_count".to_vec(),
            Op::Put(non_zero.to_be_bytes().to_vec()),
        ),
    ]
}
