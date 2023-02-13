// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use engine_traits::Result;
use proxy_ffi::interfaces_ffi;

impl From<interfaces_ffi::RawCppPtr> for crate::RawPSWriteBatchWrapper {
    fn from(src: interfaces_ffi::RawCppPtr) -> Self {
        let result = crate::RawPSWriteBatchWrapper {
            ptr: src.ptr,
            type_: src.type_,
        };
        let mut src = src;
        src.ptr = std::ptr::null_mut();
        result
    }
}

pub type RawPSWriteBatchPtr = *mut ::std::os::raw::c_void;
pub type RawPSWriteBatchWrapperTag = u32;

// This is just a copy from engine_store_ffi::RawCppPtr
#[repr(C)]
#[derive(Debug)]
pub struct RawPSWriteBatchWrapper {
    pub ptr: RawPSWriteBatchPtr,
    pub type_: RawPSWriteBatchWrapperTag,
}

unsafe impl Send for RawPSWriteBatchWrapper {}

pub trait FFIHubInner {
    fn create_write_batch(&self) -> RawPSWriteBatchWrapper;

    fn destroy_write_batch(&self, wb_wrapper: &RawPSWriteBatchWrapper);

    fn consume_write_batch(&self, wb: RawPSWriteBatchPtr);

    fn write_batch_size(&self, wb: RawPSWriteBatchPtr) -> usize;

    fn write_batch_is_empty(&self, wb: RawPSWriteBatchPtr) -> bool;

    fn write_batch_merge(&self, lwb: RawPSWriteBatchPtr, rwb: RawPSWriteBatchPtr);

    fn write_batch_clear(&self, wb: RawPSWriteBatchPtr);

    fn write_batch_put_page(&self, wb: RawPSWriteBatchPtr, page_id: &[u8], page: &[u8]);

    fn write_batch_del_page(&self, wb: RawPSWriteBatchPtr, page_id: &[u8]);

    fn read_page(&self, page_id: &[u8]) -> Option<Vec<u8>>;

    fn scan_page(
        &self,
        start_page_id: &[u8],
        end_page_id: &[u8],
        f: &mut dyn FnMut(&[u8], &[u8]) -> Result<bool>,
    );
}
