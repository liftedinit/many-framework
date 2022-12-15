use crate::migration::MIGRATIONS;
use crate::storage::event::LedgerIterator;
use crate::storage::multisig::MultisigTransactionStorage;
use linkme::distributed_slice;
use many_error::ManyError;
use many_migration::InnerMigration;
use many_modules::account::features::multisig::InfoReturn;
use many_modules::events::{EventInfo, EventLog};
use many_types::{Memo, SortOrder};
use merk::Op;

fn iter_through_events(
    storage: &merk::Merk,
) -> impl Iterator<Item = Result<(Vec<u8>, EventLog), ManyError>> + '_ {
    LedgerIterator::all_events(storage).map(|r| match r {
        Ok((k, v)) => {
            let log = minicbor::decode::<EventLog>(v.as_slice())
                .map_err(ManyError::deserialization_error)?;
            Ok((k.into(), log))
        }
        Err(e) => Err(ManyError::unknown(e)),
    })
}

fn iter_through_multisig_storage(
    storage: &merk::Merk,
) -> impl Iterator<Item = Result<(Vec<u8>, MultisigTransactionStorage), ManyError>> + '_ {
    LedgerIterator::all_multisig(storage, SortOrder::Ascending).map(|r| match r {
        Ok((k, v)) => {
            let log = minicbor::decode::<MultisigTransactionStorage>(v.as_slice())
                .map_err(ManyError::deserialization_error)?;
            Ok((k.into(), log))
        }
        Err(e) => Err(ManyError::unknown(e)),
    })
}

fn update_multisig_submit_events(storage: &mut merk::Merk) -> Result<(), ManyError> {
    let mut batch = Vec::new();

    for log in iter_through_events(storage) {
        let (key, EventLog { id, time, content }) = log?;

        if let EventInfo::AccountMultisigSubmit {
            submitter,
            account,
            memo_,
            transaction,
            token,
            threshold,
            timeout,
            execute_automatically,
            data_,
            memo,
        } = content
        {
            if memo.is_some() {
                continue;
            }
            let memo = match (memo_, data_) {
                (Some(m), Some(d)) => {
                    let mut m = Memo::from(m);
                    m.push_bytes(d.as_bytes().to_vec())?;
                    Some(m)
                }
                (Some(m), _) => Some(Memo::from(m)),
                (_, Some(d)) => Some(Memo::from(d)),
                _ => None,
            };

            if let Some(memo) = memo {
                let new_log = EventLog {
                    id,
                    time,
                    content: EventInfo::AccountMultisigSubmit {
                        submitter,
                        account,
                        memo_: None,
                        transaction,
                        token,
                        threshold,
                        timeout,
                        execute_automatically,
                        data_: None,
                        memo: Some(memo),
                    },
                };
                batch.push((
                    key,
                    Op::Put(minicbor::to_vec(new_log).map_err(ManyError::serialization_error)?),
                ));
            }
        }
    }

    // The iterator is already sorted when going through rocksdb.
    // Since we only filter and map above, the keys in batch will always
    // be sorted at this point.
    storage
        .apply(batch.as_slice())
        .map_err(ManyError::unknown)?;
    storage.commit(&[]).map_err(ManyError::unknown)?;
    Ok(())
}

fn update_multisig_storage(storage: &mut merk::Merk) -> Result<(), ManyError> {
    let mut batch = Vec::new();

    for multisig in iter_through_multisig_storage(storage) {
        let (
            key,
            MultisigTransactionStorage {
                account,
                info,
                creation,
                disabled,
            },
        ) = multisig?;

        if info.memo.is_some() {
            continue;
        }

        let new_memo = match (info.memo_, info.data_) {
            (Some(m), Some(d)) => {
                let mut memo = Memo::from(m);
                memo.push_bytes(d.as_bytes().to_vec())?;
                Some(memo)
            }
            (Some(m), _) => Some(Memo::from(m)),
            (_, Some(d)) => Some(Memo::from(d)),
            _ => None,
        };

        if let Some(memo) = new_memo {
            let new_multisig = MultisigTransactionStorage {
                account,
                creation,
                info: InfoReturn {
                    memo_: None,
                    data_: None,
                    memo: Some(memo),
                    ..info
                },
                disabled,
            };

            batch.push((
                key,
                Op::Put(minicbor::to_vec(new_multisig).map_err(ManyError::serialization_error)?),
            ));
        }
    }

    // The iterator is already sorted when going through rocksdb.
    // Since we only filter and map above, the keys in batch will always
    // be sorted at this point.
    storage
        .apply(batch.as_slice())
        .map_err(ManyError::unknown)?;
    storage.commit(&[]).map_err(ManyError::unknown)?;
    Ok(())
}

fn initialize(storage: &mut merk::Merk) -> Result<(), ManyError> {
    update_multisig_submit_events(storage)?;
    update_multisig_storage(storage)?;
    Ok(())
}

#[distributed_slice(MIGRATIONS)]
pub static MEMO_MIGRATION: InnerMigration<merk::Merk, ManyError> = InnerMigration::new_initialize(
    initialize,
    "Memo Migration",
    "Move the database from legacy memo and data to the new memo data type.",
);