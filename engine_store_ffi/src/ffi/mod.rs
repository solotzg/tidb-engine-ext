// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

/// All mods end up with `_impls` impl structs defined in interface.
/// Other mods which define and impl structs should not end up with name
/// `_impls`.

#[allow(dead_code)]
pub mod interfaces;
// All ffi impls that without raft context.
pub mod basic_ffi_impls;
// All ffi impls that with raft context, but without proxy helper context.
pub mod domain_impls;
pub mod encryption_impls;
pub mod engine_store_helper_impls;
pub(crate) mod lock_cf_reader;
pub mod raftstore_proxy_helper_impls;
pub mod sst_reader_impls;

pub use engine_tiflash::EngineStoreConfig;

use self::interfaces::root::DB::{RaftStoreProxyPtr, SSTView, SSTViewVec};
pub use self::{
    basic_ffi_impls::*,
    domain_impls::*,
    encryption_impls::*,
    engine_store_helper_impls::*,
    interfaces::root::DB::{
        BaseBuffView, ColumnFamilyType, CppStrVecView, CppStrWithView, EngineStoreApplyRes,
        EngineStoreServerHelper, EngineStoreServerStatus, FastAddPeerRes, FastAddPeerStatus,
        FileEncryptionRes, FsStats, HttpRequestRes, HttpRequestStatus, KVGetStatus,
        PageAndCppStrWithView, RaftCmdHeader, RaftProxyStatus, RaftStoreProxyFFIHelper, RawCppPtr,
        RawCppPtrArr, RawCppPtrCarr, RawCppPtrTuple, RawCppStringPtr, RawVoidPtr, SSTReaderPtr,
        SpecialCppPtrType, StoreStats, WriteCmdType, WriteCmdsView,
    },
    lock_cf_reader::*,
    raftstore_proxy_helper_impls::*,
    sst_reader_impls::*,
};
use crate::TiFlashEngine;

#[allow(clippy::wrong_self_convention)]
pub trait UnwrapExternCFunc<T> {
    unsafe fn into_inner(&self) -> &T;
}

impl<T> UnwrapExternCFunc<T> for std::option::Option<T> {
    unsafe fn into_inner(&self) -> &T {
        std::mem::transmute::<&Self, &T>(self)
    }
}
pub trait RaftStoreProxyFFI: Sync {
    fn set_status(&mut self, s: RaftProxyStatus);
    fn get_value_cf<F>(&self, cf: &str, key: &[u8], cb: F)
    where
        F: FnOnce(Result<Option<&[u8]>, String>);
    fn set_kv_engine(&mut self, kv_engine: Option<TiFlashEngine>);
}

impl RawCppPtr {
    pub fn into_raw(mut self) -> RawVoidPtr {
        let ptr = self.ptr;
        self.ptr = std::ptr::null_mut();
        ptr
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }
}

unsafe impl Send for RawCppPtr {}
// Do not guarantee raw pointer could be accessed between threads safely
// unsafe impl Sync for RawCppPtr {}

impl Drop for RawCppPtr {
    fn drop(&mut self) {
        if !self.is_null() {
            let helper = get_engine_store_server_helper();
            helper.gc_raw_cpp_ptr(self.ptr, self.type_);
            self.ptr = std::ptr::null_mut();
        }
    }
}

impl RawCppPtrTuple {
    pub fn is_null(&self) -> bool {
        unsafe { (*self.inner).ptr.is_null() }
    }
}

unsafe impl Send for RawCppPtrTuple {}

impl Drop for RawCppPtrTuple {
    fn drop(&mut self) {
        // Note the layout is:
        // [0] RawCppPtr to T
        // [1] RawCppPtr to R
        // ...
        // [len-1] RawCppPtr to S
        unsafe {
            if !self.is_null() {
                let helper = get_engine_store_server_helper();
                let len = self.len;
                // Delete all `void *`.
                for i in 0..len {
                    let i = i as usize;
                    let inner_i = self.inner.add(i);
                    // Will not fire even without the if in tests,
                    // since type must be 0 which is None.
                    if !inner_i.is_null() {
                        helper.gc_raw_cpp_ptr((*inner_i).ptr, (*inner_i).type_);
                        // We still set to nullptr, even though we will immediately delete it.
                        (*inner_i).ptr = std::ptr::null_mut();
                    }
                }
                // Delete `void **`.
                helper.gc_special_raw_cpp_ptr(
                    self.inner as RawVoidPtr,
                    self.len,
                    SpecialCppPtrType::TupleOfRawCppPtr,
                );
                self.inner = std::ptr::null_mut();
                self.len = 0;
            }
        }
    }
}

impl RawCppPtrArr {
    pub fn is_null(&self) -> bool {
        self.inner.is_null()
    }
}

unsafe impl Send for RawCppPtrArr {}

impl Drop for RawCppPtrArr {
    fn drop(&mut self) {
        // Note the layout is:
        // [0] RawVoidPtr to T
        // [1] RawVoidPtr
        // ...
        // [len-1] RawVoidPtr
        unsafe {
            if !self.is_null() {
                let helper = get_engine_store_server_helper();
                let len = self.len;
                // Delete all `T *`
                for i in 0..len {
                    let i = i as usize;
                    let inner_i = self.inner.add(i);
                    // Will fire even without the if in tests, since type is not 0.
                    if !(*inner_i).is_null() {
                        helper.gc_raw_cpp_ptr(*inner_i, self.type_);
                        // We still set to nullptr, even though we will immediately delete it.
                        *inner_i = std::ptr::null_mut();
                    }
                }
                // Delete `T **`
                helper.gc_special_raw_cpp_ptr(
                    self.inner as RawVoidPtr,
                    self.len,
                    SpecialCppPtrType::ArrayOfRawCppPtr,
                );
                self.inner = std::ptr::null_mut();
                self.len = 0;
            }
        }
    }
}

impl Drop for RawCppPtrCarr {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            let helper = get_engine_store_server_helper();
            helper.gc_raw_cpp_ptr_carr(self.inner as RawVoidPtr, self.type_, self.len);
            self.inner = std::ptr::null_mut();
            self.len = 0;
        }
    }
}

pub fn set_server_info_resp(res: &kvproto::diagnosticspb::ServerInfoResponse, ptr: RawVoidPtr) {
    get_engine_store_server_helper().set_server_info_resp(res, ptr)
}
