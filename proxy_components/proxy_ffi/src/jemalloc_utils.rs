// Copyright 2024 TiKV Project Authors. Licensed under Apache-2.0.

extern "C" {
    // External jemalloc
    pub fn mallctl(
        name: *const ::std::os::raw::c_char,
        oldp: *mut ::std::os::raw::c_void,
        oldlenp: *mut u64,
        newp: *mut ::std::os::raw::c_void,
        newlen: u64,
    ) -> ::std::os::raw::c_int;

    // Embedded jemalloc
    pub fn _rjem_mallctl(
        name: *const ::std::os::raw::c_char,
        oldp: *mut ::std::os::raw::c_void,
        oldlenp: *mut u64,
        newp: *mut ::std::os::raw::c_void,
        newlen: u64,
    ) -> ::std::os::raw::c_int;
}

fn issue_mallctl(command: &str) -> u64{
    type PtrUnderlying = u64;
    let mut ptr: PtrUnderlying = 0;
    let mut size = std::mem::size_of::<PtrUnderlying>() as u64;
    let c_str = std::ffi::CString::new(command).unwrap();
    let c_ptr: *const ::std::os::raw::c_char = c_str.as_ptr() as *const ::std::os::raw::c_char;
    unsafe {
        #[cfg(any(test, feature = "testexport"))]
        _rjem_mallctl(
            c_ptr,
            &mut ptr as *mut _ as *mut ::std::os::raw::c_void,
            &mut size as *mut u64,
            std::ptr::null_mut(),
            0,
        );

        #[cfg(not(any(test, feature = "testexport")))]
        {
            #[cfg(feature = "external-jemalloc")]
            mallctl(
                c_ptr,
                &mut ptr as *mut _ as *mut ::std::os::raw::c_void,
                &mut size as *mut u64,
                std::ptr::null_mut(),
                0,
            );
        }
    }
    ptr
}

pub fn get_allocatep_on_thread_start() -> u64 {
    issue_mallctl("thread.allocatedp")
}

pub fn get_deallocatep_on_thread_start() -> u64 {
    issue_mallctl("thread.deallocatedp")
}
