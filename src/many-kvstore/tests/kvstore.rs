pub mod common;

use crate::common::{setup, Setup};
use many_identity::testing::identity;
use many_kvstore::error;
use many_modules::kvstore::{
    DisableArgsBuilder, GetArgs, InfoArg, KvStoreCommandsModuleBackend, KvStoreModuleBackend,
    PutArgsBuilder,
};
use minicbor::bytes::ByteVec;

#[test]
fn info() {
    let Setup { module_impl, id } = setup();
    let info = module_impl.info(&id, InfoArg {});
    assert!(info.is_ok());
}

#[test]
fn put_get_disable() {
    let Setup {
        mut module_impl,
        id,
    } = setup();
    let put_data = PutArgsBuilder::default()
        .key(vec![1].into())
        .value(vec![2].into())
        .build()
        .unwrap();
    let put = module_impl.put(&id, put_data);
    assert!(put.is_ok());

    let get_data = GetArgs {
        key: vec![1].into(),
    };
    let get_value = module_impl.get(&id, get_data).unwrap().value.unwrap();
    assert_eq!(ByteVec::from(vec![2]), get_value);

    let disable_data = DisableArgsBuilder::default()
        .key(vec![1].into())
        .build()
        .unwrap();
    let disable = module_impl.disable(&id, disable_data);
    assert!(disable.is_ok());

    let get_data = GetArgs {
        key: vec![1].into(),
    };
    let get_value = module_impl.get(&id, get_data);
    assert!(get_value.is_err());
    assert_eq!(get_value.unwrap_err().code(), error::key_disabled().code());
}

#[test]
fn put_put() {
    let Setup {
        mut module_impl,
        id,
    } = setup();
    let mut put_data = PutArgsBuilder::default()
        .key(vec![1].into())
        .value(vec![2].into())
        .build()
        .unwrap();
    let get_data = GetArgs {
        key: vec![1].into(),
    };
    let put = module_impl.put(&id, put_data.clone());
    assert!(put.is_ok());

    let get_value = module_impl
        .get(&id, get_data.clone())
        .unwrap()
        .value
        .unwrap();
    assert_eq!(ByteVec::from(vec![2]), get_value);

    put_data.value = vec![3].into();
    let put = module_impl.put(&id, put_data);
    assert!(put.is_ok());

    let get_value = module_impl.get(&id, get_data).unwrap().value.unwrap();
    assert_eq!(ByteVec::from(vec![3]), get_value);
}

#[test]
fn put_put_unauthorized() {
    let Setup {
        mut module_impl,
        id,
    } = setup();
    let mut put_data = PutArgsBuilder::default()
        .key(vec![1].into())
        .value(vec![2].into())
        .build()
        .unwrap();
    let get_data = GetArgs {
        key: vec![1].into(),
    };
    let put = module_impl.put(&id, put_data.clone());
    assert!(put.is_ok());
    let get_value = module_impl.get(&id, get_data).unwrap().value.unwrap();
    assert_eq!(ByteVec::from(vec![2]), get_value);

    put_data.value = vec![3].into();
    let put = module_impl.put(&identity(1), put_data);
    assert!(put.is_err());
    assert_eq!(put.unwrap_err().code(), error::permission_denied().code());
}

#[test]
fn put_disable_unauthorized() {
    let Setup {
        mut module_impl,
        id,
    } = setup();
    let put_data = PutArgsBuilder::default()
        .key(vec![1].into())
        .value(vec![2].into())
        .build()
        .unwrap();
    let get_data = GetArgs {
        key: vec![1].into(),
    };
    let put = module_impl.put(&id, put_data.clone());
    assert!(put.is_ok());

    let get_value = module_impl.get(&id, get_data).unwrap().value.unwrap();
    assert_eq!(ByteVec::from(vec![2]), get_value);

    let disable_data = DisableArgsBuilder::default()
        .key(put_data.key)
        .build()
        .unwrap();
    let disable = module_impl.disable(&identity(1), disable_data);
    assert!(disable.is_err());
    assert_eq!(
        disable.unwrap_err().code(),
        error::permission_denied().code()
    );
}

#[test]
fn put_disable_put() {
    let Setup {
        mut module_impl,
        id,
    } = setup();
    let mut put_data = PutArgsBuilder::default()
        .key(vec![1].into())
        .value(vec![2].into())
        .build()
        .unwrap();
    let get_data = GetArgs {
        key: vec![1].into(),
    };
    let put = module_impl.put(&id, put_data.clone());
    assert!(put.is_ok());
    let get_value = module_impl
        .get(&id, get_data.clone())
        .unwrap()
        .value
        .unwrap();
    assert_eq!(ByteVec::from(vec![2]), get_value);

    let disable_data = DisableArgsBuilder::default()
        .key(put_data.clone().key)
        .build()
        .unwrap();
    let disable = module_impl.disable(&id, disable_data);
    assert!(disable.is_ok());

    let get_value = module_impl.get(&id, get_data.clone());
    assert!(get_value.is_err());
    assert_eq!(get_value.unwrap_err().code(), error::key_disabled().code());

    put_data.value = vec![3].into();
    let put = module_impl.put(&identity(1), put_data.clone());
    assert!(put.is_err());
    assert_eq!(put.unwrap_err().code(), error::permission_denied().code());

    let put = module_impl.put(&id, put_data);
    assert!(put.is_ok());

    let get_value = module_impl.get(&id, get_data).unwrap().value.unwrap();
    assert_eq!(ByteVec::from(vec![3]), get_value);
}
