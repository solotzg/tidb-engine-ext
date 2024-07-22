use std::{
    convert::TryInto,
    ffi::CString,
    os::raw::{c_char, c_int},
};

use engine_store_ffi::ffi::get_engine_store_server_helper;

/// # Safety
/// Print version infomatin to std output.
#[no_mangle]
pub unsafe extern "C" fn print_raftstore_proxy_version() {
    proxy_server::print_proxy_version();
}

/// # Safety
/// Please make sure such function will be run in an independent thread. Usage
/// about interfaces can be found in `struct EngineStoreServerHelper`.
#[no_mangle]
pub unsafe extern "C" fn run_raftstore_proxy_ffi(
    argc: c_int,
    argv: *const *const c_char,
    helper: *const u8,
) {
    proxy_server::run_proxy(argc, argv, helper);
}

fn main() {
    use tempfile::{NamedTempFile};
    use std::path::Path;

    let f = NamedTempFile::new().unwrap();
    let path = f.path().to_str().unwrap();
    println!("!!!!! XXXX {}", path);
    proxy_server::status_server::vendored_utils::adhoc_dump(path).unwrap();
    let target_path = Path::new(path);
    assert_eq!(target_path.exists(), true);
    std::thread::sleep(std::time::Duration::from_millis(100000));
    panic!("AAAAA");

    // unsafe {
    //     let args: Vec<String> = std::env::args().collect();
    //     let a: Vec<CString> = args
    //         .iter()
    //         .map(|e| {
    //             let c_str = CString::new(e.as_str()).unwrap();
    //             c_str
    //         })
    //         .collect();
    //     let b: Vec<*const c_char> = a
    //         .iter()
    //         .map(|c_str| {
    //             let c_world: *const c_char = c_str.as_ptr() as *const c_char;
    //             c_world
    //         })
    //         .collect();
    //     let (_, ptr) = make_global_ffi_helper_set_no_bind();
    //     engine_store_ffi::ffi::init_engine_store_server_helper(ptr);
    //     let helper = get_engine_store_server_helper();
    //     run_raftstore_proxy_ffi(
    //         args.len().try_into().unwrap(),
    //         b.as_ptr(),
    //         helper as *const _ as *const u8,
    //     );
    // }
}
