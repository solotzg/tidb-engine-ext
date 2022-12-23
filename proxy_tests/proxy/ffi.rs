// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use core::slice::SlicePattern;

use engine_store_ffi::{
    get_engine_store_server_helper, EngineStoreServerHelper, RawCppPtr, RawCppPtrArr,
    UnwrapExternCFunc,
};
use new_mock_engine_store::mock_cluster::init_global_ffi_helper_set;

#[test]
fn test_array_of_raw_cpp_ptr() {
    println!("!!!! A 0");
    tikv_util::set_panic_hook(true, "./");
    unsafe {
        println!("!!!! A 1");
        init_global_ffi_helper_set();
        println!("!!!! A 2");
        let helper = get_engine_store_server_helper();

        let len = 10;
        let mut v: Vec<RawCppPtr> = vec![];

        println!("!!!! A 3");
        for i in 0..len {
            let s = format!("s{}", i);
            v.push((helper.fn_gen_cpp_string.into_inner())(s.as_bytes().into()));
        }

        println!("!!!! A 4");

        let (ptr_v, l, cap) = v.into_raw_parts();
        let cpp_ptr_arr = RawCppPtrArr {
            inner: ptr_v,
            len: cap as u64,
        };

        println!("!!!! A 5");
        drop(cpp_ptr_arr);
    }
}
