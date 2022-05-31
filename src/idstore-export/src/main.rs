use clap::{ArgGroup, Parser};
use merk::rocksdb::{Direction, IteratorMode, ReadOptions};
use merk::tree::Tree;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(crate) const IDSTORE_ROOT: &[u8] = b"/idstore/";

#[derive(Parser)]
#[clap(
    group(
        ArgGroup::new("hsm")
        .multiple(true)
        .args(&["module", "slot", "keyid"])
        .requires_all(&["module", "slot", "keyid"])
    )
)]
struct Opts {
    /// The RocksDB store to load.
    store: PathBuf,
}

fn main() {
    let Opts { store } = Opts::parse();

    let merk = merk::Merk::open(&store).expect("Could not open the store.");

    let mut upper_bound = IDSTORE_ROOT.to_vec();
    *upper_bound.last_mut().expect("Unreachable") += 1;

    let mut opts = ReadOptions::default();
    opts.set_iterate_upper_bound(upper_bound);
    let it = merk.iter_opt(IteratorMode::From(IDSTORE_ROOT, Direction::Forward), opts);

    let mut idstore = BTreeMap::new();
    for (key, value) in it {
        let new_v = Tree::decode(key.to_vec(), value.as_ref());
        let value = new_v.value().to_vec();

        idstore.insert(base64::encode(key.as_ref()), base64::encode(&value));
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&idstore).expect("Could not serialize")
    );
}
