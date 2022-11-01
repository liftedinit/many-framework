use crate::storage::LedgerStorage;
use many_error::ManyError;
use many_identity::Address;
use many_modules::idstore;
use merk::Op;

pub(crate) const IDSTORE_ROOT: &[u8] = b"/idstore/";

#[derive(Clone, minicbor::Encode, minicbor::Decode)]
#[cbor(map)]
struct CredentialStorage {
    #[n(0)]
    cred_id: idstore::CredentialId,

    #[n(1)]
    public_key: idstore::PublicKey,
}

enum IdStoreRootSeparator {
    RecallPhrase,
    Address,
}

impl IdStoreRootSeparator {
    fn value(&self) -> &[u8] {
        match *self {
            IdStoreRootSeparator::RecallPhrase => b"00",
            IdStoreRootSeparator::Address => b"01",
        }
    }
}

impl LedgerStorage<'_> {
    pub(crate) fn inc_idstore_seed(&mut self) -> u64 {
        let idstore_seed = self
            .persistent_store
            .get(b"/config/idstore_seed")
            .unwrap()
            .map_or(0u64, |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                u64::from_be_bytes(bytes)
            });

        self.persistent_store
            .apply(&[(
                b"/config/idstore_seed".to_vec(),
                Op::Put((idstore_seed + 1).to_be_bytes().to_vec()),
            )])
            .unwrap();

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }

        idstore_seed
    }

    pub fn store(
        &mut self,
        recall_phrase: &idstore::RecallPhrase,
        address: &Address,
        cred_id: idstore::CredentialId,
        public_key: idstore::PublicKey,
    ) -> Result<(), ManyError> {
        let recall_phrase_cbor = minicbor::to_vec(recall_phrase)
            .map_err(|e| ManyError::serialization_error(e.to_string()))?;
        if self
            .persistent_store
            .get(&recall_phrase_cbor)
            .map_err(|e| ManyError::unknown(e.to_string()))?
            .is_some()
        {
            return Err(idstore::existing_entry());
        }
        let value = minicbor::to_vec(CredentialStorage {
            cred_id,
            public_key,
        })
        .map_err(|e| ManyError::serialization_error(e.to_string()))?;

        let batch = vec![
            (
                vec![
                    IDSTORE_ROOT,
                    IdStoreRootSeparator::RecallPhrase.value(),
                    &recall_phrase_cbor,
                ]
                .concat(),
                Op::Put(value.clone()),
            ),
            (
                vec![
                    IDSTORE_ROOT,
                    IdStoreRootSeparator::Address.value(),
                    &address.to_vec(),
                ]
                .concat(),
                Op::Put(value),
            ),
        ];

        self.persistent_store.apply(&batch).unwrap();

        if !self.blockchain {
            self.persistent_store
                .commit(&[])
                .expect("Could not commit to store.");
        }

        Ok(())
    }

    fn get_from_storage(
        &self,
        key: &Vec<u8>,
        sep: IdStoreRootSeparator,
    ) -> Result<Option<Vec<u8>>, ManyError> {
        self.persistent_store
            .get(&vec![IDSTORE_ROOT, sep.value(), key].concat())
            .map_err(|e| ManyError::unknown(e.to_string()))
    }

    pub fn get_from_recall_phrase(
        &self,
        recall_phrase: &idstore::RecallPhrase,
    ) -> Result<(idstore::CredentialId, idstore::PublicKey), ManyError> {
        let recall_phrase_cbor = minicbor::to_vec(recall_phrase)
            .map_err(|e| ManyError::serialization_error(e.to_string()))?;
        if let Some(value) =
            self.get_from_storage(&recall_phrase_cbor, IdStoreRootSeparator::RecallPhrase)?
        {
            let value: CredentialStorage = minicbor::decode(&value)
                .map_err(|e| ManyError::deserialization_error(e.to_string()))?;
            Ok((value.cred_id, value.public_key))
        } else {
            Err(idstore::entry_not_found(recall_phrase.join(" ")))
        }
    }

    pub fn get_from_address(
        &self,
        address: &Address,
    ) -> Result<(idstore::CredentialId, idstore::PublicKey), ManyError> {
        if let Some(value) =
            self.get_from_storage(&address.to_vec(), IdStoreRootSeparator::Address)?
        {
            let value: CredentialStorage = minicbor::decode(&value)
                .map_err(|e| ManyError::deserialization_error(e.to_string()))?;
            Ok((value.cred_id, value.public_key))
        } else {
            Err(idstore::entry_not_found(address.to_string()))
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    impl LedgerStorage<'_> {
        pub fn set_idstore_seed(&mut self, seed: u64) {
            self.persistent_store
                .apply(&[(
                    b"/config/idstore_seed".to_vec(),
                    Op::Put(seed.to_be_bytes().to_vec()),
                )])
                .unwrap();

            self.persistent_store.commit(&[]).unwrap();
        }
    }
}
