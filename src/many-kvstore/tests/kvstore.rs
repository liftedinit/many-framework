pub mod common;
use crate::common::{setup, Setup};
use minicbor::bytes::ByteVec;

use many::server::module::kvstore::{
    DeleteArgs, GetArgs, InfoArg, KvStoreCommandsModuleBackend, KvStoreModuleBackend, PutArgs,
};

#[test]
fn kvstore_info() {
    let Setup { module_impl, id } = setup();
    let info = module_impl.info(&id, InfoArg {});
    assert!(info.is_ok());

    let info_hash = info.unwrap().hash;
    assert!(info_hash.len() > 1);
}

#[test]
fn kvstore_put_get_delete() {
    let Setup {
        mut module_impl,
        id,
    } = setup();
    let put_data = PutArgs {
        key: vec![1].into(),
        value: vec![2].into(),
    };
    let put = module_impl.put(&id, put_data);
    assert!(put.is_ok());

    let get_data = GetArgs {
        key: vec![1].into(),
    };
    let get_value = module_impl.get(&id, get_data).unwrap().value.unwrap();
    assert_eq!(ByteVec::from(vec![2]), get_value);

    let delete_data = DeleteArgs {
        key: vec![1].into(),
    };
    let delete = module_impl.delete(&id, delete_data);
    assert!(delete.is_ok());

    let get_data = GetArgs {
        key: vec![1].into(),
    };
    let get_value = module_impl.get(&id, get_data).unwrap().value;
    assert_eq!(get_value, None);
}
