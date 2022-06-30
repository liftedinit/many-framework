use many::{
    server::module::{
        idstore::{CredentialId},
    },
    types::{
        identity::{cose::testsutils::generate_random_eddsa_identity,         },
    },
    Identity, 
};
use many_kvstore::module::{KvStoreModuleImpl};

pub struct Setup {
    pub module_impl: KvStoreModuleImpl,
    pub id: Identity,
    pub cred_id: CredentialId,
    time: Option<u64>,
}

// impl Default for Setup {
//     fn default(persistence: &str) -> Self {
//         Self::new(persistence, false)
//     }
// }

impl Setup {
    pub fn new(persistence: &str, blockchain: bool) -> Self {
        let _ = std::fs::remove_dir_all(persistence);
        let id = generate_random_eddsa_identity();
        let state = serde_json::from_str("{\"acls\": {},\"hash\": \"f187041ec9ef4fca803cf536943f537f8fec76ad6b0507edab8a44408d331bc4\"}").unwrap();
        
        Self {
            module_impl: KvStoreModuleImpl::new(
                state,
                persistence,
                blockchain
            ).unwrap(),
            id: id.identity,
            cred_id: CredentialId(vec![1; 16].into()),
            time: Some(1_000_000),
        }
    }

    pub fn load(persistence: &str, blockchain: bool) -> Self {
        let id = generate_random_eddsa_identity();
        
        Self {
            module_impl: KvStoreModuleImpl::load(
                persistence,
                blockchain
            ).unwrap(),
            id: id.identity,
            cred_id: CredentialId(vec![1; 16].into()),
            time: Some(1_000_000),
        }
    }
}

pub fn setup(persistence: &str) -> Setup {
    Setup::new(persistence, false)
}

pub fn setup_from_load(persistence: &str) -> Setup {
    Setup::load(persistence, false)
}