extern crate core;

use clap::Parser;
use merk::rocksdb::{Direction, IteratorMode, ReadOptions};
use merk::tree::Tree;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(crate) const IDSTORE_ROOT: &[u8] = b"/idstore/";

#[derive(Parser)]
struct Opts {
    /// The RocksDB store to load.
    store: PathBuf,
}

#[derive(serde_derive::Serialize)]
struct JsonRoot {
    seed: u64,
    keys: BTreeMap<String, String>,
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

    let root = JsonRoot {
        seed: merk
            .get(b"/config/idstore_seed")
            .expect("Could not read see")
            .map_or(0u64, |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                u64::from_be_bytes(bytes)
            }),
        keys: idstore,
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&root).expect("Could not serialize")
    );
}
