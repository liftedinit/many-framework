use std::collections::BTreeMap;

use super::KvStoreModuleImpl;
use many_error::ManyError;
use many_identity::Address;
use many_modules::account::features::multisig::MultisigTransactionState;
use many_modules::events::{
    self, EventFilterAttributeSpecific, EventFilterAttributeSpecificIndex, EventInfo, EventLog,
};
use many_types::{CborRange, Timestamp, VecOrSingle};

const MAXIMUM_EVENT_COUNT: usize = 100;

impl events::EventsModuleBackend for KvStoreModuleImpl {
    fn info(&self, _args: events::InfoArgs) -> Result<events::InfoReturn, ManyError> {
        use strum::IntoEnumIterator;
        Ok(events::InfoReturn {
            total: self.storage.nb_events(),
            event_types: events::EventKind::iter().collect(),
        })
    }

    fn list(&self, args: events::ListArgs) -> Result<events::ListReturns, ManyError> {
        let events::ListArgs {
            count,
            order,
            filter,
        } = args;
        let filter = filter.unwrap_or_default();

        let count = count.map_or(MAXIMUM_EVENT_COUNT, |c| {
            std::cmp::min(c as usize, MAXIMUM_EVENT_COUNT)
        });

        let storage = &self.storage;
        let nb_events = storage.nb_events();
        let iter = storage.iter(
            filter.id_range.unwrap_or_default(),
            order.unwrap_or_default(),
        );

        let iter = Box::new(iter.map(|item| {
            let (_k, v) = item.map_err(|e| ManyError::unknown(e.to_string()))?;
            minicbor::decode::<events::EventLog>(v.as_slice())
                .map_err(|e| ManyError::deserialization_error(e.to_string()))
        }));

        let iter = filter_account(iter, filter.account);
        let iter = filter_event_kind(iter, filter.kind);
        let iter = filter_date(iter, filter.date_range.unwrap_or_default());
        let iter = filter_attribute_specific(iter, &filter.events_filter_attribute_specific);

        let events: Vec<events::EventLog> = iter.take(count).collect::<Result<_, _>>()?;

        Ok(events::ListReturns { nb_events, events })
    }
}

type EventLogResult = Result<events::EventLog, ManyError>;

fn filter_account<'a>(
    it: Box<dyn Iterator<Item = EventLogResult> + 'a>,
    account: Option<VecOrSingle<Address>>,
) -> Box<dyn Iterator<Item = EventLogResult> + 'a> {
    if let Some(account) = account {
        let account: Vec<Address> = account.into();
        Box::new(it.filter(move |t| match t {
            // Propagate the errors.
            Err(_) => true,
            Ok(t) => account.iter().any(|id| t.is_about(id)),
        }))
    } else {
        it
    }
}

fn filter_event_kind<'a>(
    it: Box<dyn Iterator<Item = EventLogResult> + 'a>,
    event_kind: Option<VecOrSingle<events::EventKind>>,
) -> Box<dyn Iterator<Item = EventLogResult> + 'a> {
    if let Some(k) = event_kind {
        let k: Vec<events::EventKind> = k.into();
        Box::new(it.filter(move |t| match t {
            Err(_) => true,
            Ok(t) => k.contains(&t.kind()),
        }))
    } else {
        it
    }
}

fn filter_date<'a>(
    it: Box<dyn Iterator<Item = EventLogResult> + 'a>,
    range: CborRange<Timestamp>,
) -> Box<dyn Iterator<Item = EventLogResult> + 'a> {
    Box::new(it.filter(move |t| match t {
        // Propagate the errors.
        Err(_) => true,
        Ok(events::EventLog { time, .. }) => range.contains(time),
    }))
}

fn filter_attribute_specific<'a>(
    mut it: Box<dyn Iterator<Item = EventLogResult> + 'a>,
    attribute_specific: &'a BTreeMap<
        EventFilterAttributeSpecificIndex,
        EventFilterAttributeSpecific,
    >,
) -> Box<dyn Iterator<Item = EventLogResult> + 'a> {
    for x in attribute_specific.values() {
        match x {
            EventFilterAttributeSpecific::MultisigTransactionState(VecOrSingle(state)) => {
                it = Box::new(it.filter(|t| match t {
                    Err(_) => true,
                    Ok(EventLog {
                        content: EventInfo::AccountMultisigSubmit { .. },
                        ..
                    })
                    | Ok(EventLog {
                        content: EventInfo::AccountMultisigApprove { .. },
                        ..
                    }) => state.contains(&MultisigTransactionState::Pending),
                    Ok(EventLog {
                        content: EventInfo::AccountMultisigExecute { .. },
                        ..
                    }) => {
                        state.contains(&MultisigTransactionState::ExecutedAutomatically)
                            || state.contains(&MultisigTransactionState::ExecutedManually)
                    }
                    Ok(EventLog {
                        content: EventInfo::AccountMultisigWithdraw { .. },
                        ..
                    }) => state.contains(&MultisigTransactionState::Withdrawn),
                    Ok(EventLog {
                        content: EventInfo::AccountMultisigExpired { .. },
                        ..
                    }) => state.contains(&MultisigTransactionState::Expired),
                    _ => false,
                }))
            }
        }
    }
    it
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use many_error::ManyError;
    use many_identity::Address;
    use many_modules::account::features::multisig::MultisigTransactionState;
    use many_modules::events::{
        EventFilterAttributeSpecific, EventFilterAttributeSpecificIndex, EventId, EventInfo,
        EventLog,
    };
    use many_types::{Timestamp, VecOrSingle};
    use minicbor::bytes::ByteVec;

    use super::{filter_attribute_specific, EventLogResult};

    #[test]
    fn test_filter_attribute_specific() {
        let eventlogs: Vec<EventLogResult> = vec![
            Ok(EventLog {
                id: EventId::from(vec![0]),
                time: Timestamp::new(0).unwrap(),
                content: EventInfo::AccountMultisigExpired {
                    account: Address::anonymous(),
                    token: ByteVec::from(vec![0]),
                    time: Timestamp::new(0).unwrap(),
                },
            }),
            Err(ManyError::default()),
            Ok(EventLog {
                id: EventId::from(vec![1]),
                time: Timestamp::now(),
                content: EventInfo::AccountMultisigWithdraw {
                    account: Address::anonymous(),
                    token: ByteVec::from(vec![0]),
                    withdrawer: Address::anonymous(),
                },
            }),
        ];
        let filter = BTreeMap::from([(
            EventFilterAttributeSpecificIndex::MultisigTransactionState,
            EventFilterAttributeSpecific::MultisigTransactionState(VecOrSingle(vec![
                MultisigTransactionState::Expired,
            ])),
        )]);
        let iter = Box::new(eventlogs.into_iter());
        let iter = filter_attribute_specific(iter, &filter);
        let filtered_eventlogs: Vec<EventLogResult> = iter.collect();
        assert_eq!(filtered_eventlogs.len(), 2);
        assert_eq!(
            filtered_eventlogs[0].as_ref().unwrap().id,
            EventId::from(vec![0])
        );
        assert_eq!(
            filtered_eventlogs[1].as_ref().unwrap_err(),
            &ManyError::default()
        );
    }
}
