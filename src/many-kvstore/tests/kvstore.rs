pub mod common;
use crate::common::{setup, setup_from_load, Setup};
use minicbor::bytes::ByteVec;

use many::{
    server::module::kvstore::{
        KvStoreModuleBackend, 
        KvStoreCommandsModuleBackend, 
        InfoArg,
        PutArgs,
        GetArgs,
        DeleteArgs
    },
};

#[test]
fn kvstore_info() {
    let Setup {
        module_impl, 
        id, 
        ..
    } = setup("../somepath.db");
    
    let result = module_impl.info(&id, InfoArg {});
    assert!(result.is_ok());
}

#[test]
fn kvstore_put() {
    let Setup {
        mut module_impl, 
        id, 
        ..
    } = setup("../somepath2.db");

    let data = PutArgs {
        key: ByteVec::from(vec![1]),
        value: ByteVec::from(vec![2]),
    };
    
    let result = module_impl.put(&id, data);
    assert!(result.is_ok());
}

#[test]
fn kvstore_get() {
    let Setup {
        module_impl, 
        id, 
        ..
    } = setup("../somepath3.db");

    let data = GetArgs {
        key: ByteVec::from(vec![1]),
    };
    
    let result = module_impl.get(&id, data);
    assert!(result.is_ok());
}

#[test]
fn kvstore_delete() {
    let Setup {
        mut module_impl, 
        id, 
        ..
    } = setup("../somepath4.db");

    let data = DeleteArgs {
        key: ByteVec::from(vec![1]),
    };
    
    let result = module_impl.delete(&id, data);
    assert!(result.is_ok());
}

#[test]
fn kvstore_load() {
    setup("../somepath5.db");

    let Setup {
        module_impl, 
        id, 
        ..
    } = setup_from_load("../somepath5.db");
    
    let result = module_impl.info(&id, InfoArg {});
    assert!(result.is_ok());
}