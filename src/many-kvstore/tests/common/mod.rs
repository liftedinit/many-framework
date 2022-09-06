use many_identity::testsutils::generate_random_eddsa_identity;
use many_identity::Address;
use many_kvstore::module::KvStoreModuleImpl;

pub struct Setup {
    pub module_impl: KvStoreModuleImpl,
    pub id: Address,
}

impl Default for Setup {
    fn default() -> Self {
        Self::new(false)
    }
}

impl Setup {
    pub fn new(blockchain: bool) -> Self {
        let id = generate_random_eddsa_identity();
        let content = std::fs::read_to_string("../../staging/kvstore_state.json").unwrap();
        let state = serde_json::from_str(&content).unwrap();
        Self {
            module_impl: KvStoreModuleImpl::new(state, tempfile::tempdir().unwrap(), blockchain)
                .unwrap(),
            id: id.identity,
        }
    }
}

pub fn setup() -> Setup {
    Setup::default()
}
