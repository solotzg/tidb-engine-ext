// Copyright 2024 TiKV Project Authors. Licensed under Apache-2.0.

use proxy_ffi::jemalloc_utils::issue_mallctl_args;
use tikv_alloc::error::ProfResult;

pub fn activate_prof() -> ProfResult<()> {
    {
        let mut value: bool = true;
        let len = std::mem::size_of_val(&value) as u64;
        issue_mallctl_args(
            "prof.active",
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut value as *mut _ as *mut _,
            len,
        );
    }
    Ok(())
}

pub fn has_activate_prof() -> bool {
    let mut value: bool = false;
    let mut len = std::mem::size_of_val(&value) as u64;
    issue_mallctl_args(
        "prof.active",
        &mut value as *mut _ as *mut _,
        &mut len as *mut _ as *mut _,
        std::ptr::null_mut(),
        0,
    );
    value
}

pub fn deactivate_prof() -> ProfResult<()> {
    {
        let mut value: bool = false;
        let len = std::mem::size_of_val(&value) as u64;
        issue_mallctl_args(
            "prof.active",
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut value as *mut _ as *mut _,
            len,
        );
    }
    Ok(())
}

pub fn dump_prof(path: &str) -> tikv_alloc::error::ProfResult<()> {
    {
        let mut bytes = std::ffi::CString::new(path)?.into_bytes_with_nul();
        let mut ptr = bytes.as_mut_ptr() as *mut ::std::os::raw::c_char;
        let len = std::mem::size_of_val(&ptr) as u64;
        issue_mallctl_args(
            "prof.dump",
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut ptr as *mut _ as *mut _,
            len,
        );
    }
    Ok(())
}

pub fn adhoc_dump(path: &str) -> tikv_alloc::error::ProfResult<()> {
    let prev_has_activate_prof = has_activate_prof();
    if !prev_has_activate_prof{
        println!("!!!!! adhoc_dump 1 {}", path);
        activate_prof();
    }
    
    println!("!!!!! adhoc_dump sleep {}", path);
    let x = vec![1; 100000];
    let y = vec![1; 100000];
    let z = vec![1; 100000];
    std::thread::sleep(std::time::Duration::from_millis(10000));
    {
        println!("!!!!! adhoc_dump 2 {}", path);
        let mut bytes = std::ffi::CString::new(path)?.into_bytes_with_nul();
        let mut ptr = bytes.as_mut_ptr() as *mut ::std::os::raw::c_char;
        let len = std::mem::size_of_val(&ptr) as u64;
        let r = issue_mallctl_args(
            "prof.dump",
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut ptr as *mut _ as *mut _,
            len,
        );
        println!("!!!!! adhoc_dump 3 {}", path);
    }
    if !prev_has_activate_prof{
        println!("!!!!! adhoc_dump 4 {} {}", path, x.len() + y.len() + z.len());
        deactivate_prof();
    }
    Ok(())
}
