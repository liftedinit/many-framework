use crate::storage::LedgerStorage;
use many_modules::events;
use many_modules::events::EventId;
use many_types::{CborRange, SortOrder};
use merk::rocksdb::{Direction, ReadOptions};
use merk::tree::Tree;
use merk::{rocksdb, Op};
use std::ops::{Bound, RangeBounds};

pub(crate) const EVENTS_ROOT: &[u8] = b"/events/";

// Left-shift the height by this amount of bits
pub(crate) const HEIGHT_EVENTID_SHIFT: u64 = 32;

/// Number of bytes in an event ID when serialized. Keys smaller than this
/// will have `\0` prepended, and keys larger will be cut to this number of
/// bytes.
pub(crate) const EVENT_ID_KEY_SIZE_IN_BYTES: usize = 32;

/// Returns the storage key for an event in the kv-store.
pub(super) fn key_for_event(id: events::EventId) -> Vec<u8> {
    let id = id.as_ref();
    let id = if id.len() > EVENT_ID_KEY_SIZE_IN_BYTES {
        &id[0..EVENT_ID_KEY_SIZE_IN_BYTES]
    } else {
        id
    };

    let mut exp_id = [0u8; EVENT_ID_KEY_SIZE_IN_BYTES];
    exp_id[(EVENT_ID_KEY_SIZE_IN_BYTES - id.len())..].copy_from_slice(id);
    vec![EVENTS_ROOT.to_vec(), exp_id.to_vec()].concat()
}

impl LedgerStorage {
    pub(crate) fn new_event_id(&mut self) -> events::EventId {
        self.latest_tid += 1;
        self.latest_tid.clone()
    }

    pub fn nb_events(&self) -> u64 {
        self.persistent_store
            .get(b"/events_count")
            .unwrap()
            .map_or(0, |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                u64::from_be_bytes(bytes)
            })
    }

    pub(crate) fn log_event(&mut self, content: events::EventInfo) {
        let current_nb_events = self.nb_events();
        let event = events::EventLog {
            id: self.new_event_id(),
            time: self.now(),
            content,
        };

        self.persistent_store
            .apply(&[
                (
                    key_for_event(event.id.clone()),
                    Op::Put(minicbor::to_vec(&event).unwrap()),
                ),
                (
                    b"/events_count".to_vec(),
                    Op::Put((current_nb_events + 1).to_be_bytes().to_vec()),
                ),
            ])
            .unwrap();

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }
    }

    pub fn iter_multisig(&self, order: SortOrder) -> LedgerIterator {
        LedgerIterator::all_multisig(&self.persistent_store, order)
    }

    pub fn iter_events(&self, range: CborRange<EventId>, order: SortOrder) -> LedgerIterator {
        LedgerIterator::events_scoped_by_id(&self.persistent_store, range, order)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use many_modules::events::EventId;

    #[test]
    fn event_key_size() {
        let golden_size = key_for_event(events::EventId::from(0)).len();

        assert_eq!(golden_size, key_for_event(EventId::from(u64::MAX)).len());

        // Test at 1 byte, 2 bytes and 4 bytes boundaries.
        for i in [u8::MAX as u64, u16::MAX as u64, u32::MAX as u64] {
            assert_eq!(golden_size, key_for_event(EventId::from(i - 1)).len());
            assert_eq!(golden_size, key_for_event(EventId::from(i)).len());
            assert_eq!(golden_size, key_for_event(EventId::from(i + 1)).len());
        }

        assert_eq!(
            golden_size,
            key_for_event(EventId::from(b"012345678901234567890123456789".to_vec())).len()
        );

        // Trim the Event ID if it's too long.
        assert_eq!(
            golden_size,
            key_for_event(EventId::from(
                b"0123456789012345678901234567890123456789".to_vec()
            ))
            .len()
        );
        assert_eq!(
            key_for_event(EventId::from(b"01234567890123456789012345678901".to_vec())).len(),
            key_for_event(EventId::from(
                b"0123456789012345678901234567890123456789012345678901234567890123456789".to_vec()
            ))
            .len()
        )
    }
}

pub struct LedgerIterator<'a> {
    inner: rocksdb::DBIterator<'a>,
}

impl<'a> LedgerIterator<'a> {
    pub fn all_multisig(merk: &'a merk::Merk, order: SortOrder) -> Self {
        use crate::storage::multisig::MULTISIG_TRANSACTIONS_ROOT;
        use rocksdb::IteratorMode;

        // Set the iterator bounds to iterate all multisig transactions.
        // We will break the loop later if we can.
        let mut options = ReadOptions::default();
        options.set_iterate_lower_bound(MULTISIG_TRANSACTIONS_ROOT);

        let mut bound = MULTISIG_TRANSACTIONS_ROOT.to_vec();
        bound[MULTISIG_TRANSACTIONS_ROOT.len() - 1] += 1;
        options.set_iterate_upper_bound(bound.clone());

        let it_mode = match order {
            SortOrder::Indeterminate | SortOrder::Ascending => {
                IteratorMode::From(MULTISIG_TRANSACTIONS_ROOT, Direction::Forward)
            }
            SortOrder::Descending => IteratorMode::From(&bound, Direction::Reverse),
        };

        let inner = merk.iter_opt(it_mode, options);

        Self { inner }
    }

    pub fn all_events(merk: &'a merk::Merk) -> Self {
        Self::events_scoped_by_id(merk, CborRange::default(), SortOrder::Indeterminate)
    }

    pub fn events_scoped_by_id(
        merk: &'a merk::Merk,
        range: CborRange<EventId>,
        order: SortOrder,
    ) -> Self {
        use rocksdb::IteratorMode;
        let mut opts = ReadOptions::default();

        match range.start_bound() {
            Bound::Included(x) => opts.set_iterate_lower_bound(key_for_event(x.clone())),
            Bound::Excluded(x) => opts.set_iterate_lower_bound(key_for_event(x.clone() + 1)),
            Bound::Unbounded => opts.set_iterate_lower_bound(EVENTS_ROOT),
        }
        match range.end_bound() {
            Bound::Included(x) => opts.set_iterate_upper_bound(key_for_event(x.clone() + 1)),
            Bound::Excluded(x) => opts.set_iterate_upper_bound(key_for_event(x.clone())),
            Bound::Unbounded => {
                let mut bound = EVENTS_ROOT.to_vec();
                bound[EVENTS_ROOT.len() - 1] += 1;
                opts.set_iterate_upper_bound(bound);
            }
        }

        let mode = match order {
            SortOrder::Indeterminate | SortOrder::Ascending => IteratorMode::Start,
            SortOrder::Descending => IteratorMode::End,
        };

        Self {
            inner: merk.iter_opt(mode, opts),
        }
    }
}

impl<'a> Iterator for LedgerIterator<'a> {
    type Item = Result<(Box<[u8]>, Vec<u8>), merk::rocksdb::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|item| {
            item.map(|(k, v)| {
                let new_v = Tree::decode(k.to_vec(), v.as_ref());

                (k, new_v.value().to_vec())
            })
        })
    }
}
