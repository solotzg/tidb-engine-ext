// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

/// All mods end up with `_impls` impl structs defined in interface.
/// Other mods which define and impl structs should not end up with name
/// `_impls`.

#[allow(dead_code)]
pub mod interfaces;
// All ffi impls that without raft domain.
pub mod basic_ffi_impls;
// All ffi impls that within raft domain, but without proxy helper context.
pub mod domain_impls;
// All ffi impls that within raft domain and proxy helper context.
pub mod context_impls;
pub mod encryption_impls;
// FFI directly related with EngineStoreServerHelper.
pub mod engine_store_helper_impls;
pub(crate) mod lock_cf_reader;
// FFI directly related with RaftStoreProxyFFIHelper.
pub mod raftstore_proxy_helper_impls;
pub mod sst_reader_impls;

pub use engine_tiflash::EngineStoreConfig;

pub use self::{
    basic_ffi_impls::*, domain_impls::*, encryption_impls::*, engine_store_helper_impls::*,
    lock_cf_reader::*, raftstore_proxy_helper_impls::*, sst_reader_impls::*,
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

pub fn set_server_info_resp(res: &kvproto::diagnosticspb::ServerInfoResponse, ptr: RawVoidPtr) {
    get_engine_store_server_helper().set_server_info_resp(res, ptr)
}
