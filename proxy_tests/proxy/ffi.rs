// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use core::slice::SlicePattern;

use engine_store_ffi::{
    get_engine_store_server_helper, EngineStoreServerHelper, RawCppPtr, RawCppPtrArr,
    RawCppPtrTuple, RawVoidPtr, UnwrapExternCFunc,
};
use new_mock_engine_store::{
    mock_cluster::init_global_ffi_helper_set, mock_store::RawCppPtrTypeImpl,
};

#[test]
fn test_tuple_of_raw_cpp_ptr() {
    tikv_util::set_panic_hook(true, "./");
    unsafe {
        init_global_ffi_helper_set();
        let helper = get_engine_store_server_helper();

        let len = 10;
        let mut v: Vec<RawCppPtr> = vec![];

        for i in 0..len {
            let s = format!("s{}", i);
            v.push((helper.fn_gen_cpp_string.into_inner())(s.as_bytes().into()));
        }

        let (ptr_v, l, cap) = v.into_raw_parts();
        let cpp_ptr_tp = RawCppPtrTuple {
            inner: ptr_v,
            len: cap as u64,
        };
        drop(cpp_ptr_tp);
    }
}

#[test]
fn test_array_of_raw_cpp_ptr() {
    tikv_util::set_panic_hook(true, "./");
    unsafe {
        init_global_ffi_helper_set();
        let helper = get_engine_store_server_helper();

        let len = 10;
        let mut v: Vec<RawVoidPtr> = vec![];

        println!("AAAA 1");
        for i in 0..len {
            let s = format!("s{}", i);
            let raw_cpp_ptr = (helper.fn_gen_cpp_string.into_inner())(s.as_bytes().into());
            let raw_void_ptr = raw_cpp_ptr.into_raw();
            v.push(raw_void_ptr);
        }

        println!("AAAA 2");
        let (ptr_v, l, cap) = v.into_raw_parts();
        println!("AAAA cap {}", cap);
        let cpp_ptr_arr = RawCppPtrArr {
            inner: ptr_v,
            type_: RawCppPtrTypeImpl::String.into(),
            len: cap as u64,
        };

        println!("AAAA 3");
        drop(cpp_ptr_arr);
        println!("AAAA 4");
    }
}
