use coset::CborSerializable;
use many::{
    server::module::idstore::{CredentialId, PublicKey},
    types::identity::{cose::testsutils::generate_random_eddsa_identity, CoseKeyIdentity},
};
use many_ledger::module::LedgerModuleImpl;

/// Setup a new identity, credential ID, public key and ledger module implementation
pub fn setup() -> (CoseKeyIdentity, CredentialId, PublicKey, LedgerModuleImpl) {
    let id = generate_random_eddsa_identity();
    let public_key = PublicKey(id.clone().key.unwrap().to_vec().unwrap().into());
    (
        id,
        CredentialId(vec![1; 16].into()),
        public_key,
        LedgerModuleImpl::new(
            Some(
                serde_json::from_str(
                    &std::fs::read_to_string("../../staging/ledger_state.json").unwrap(),
                )
                .unwrap(),
            ),
            tempfile::tempdir().unwrap(),
            false,
        )
        .unwrap(),
    )
}
