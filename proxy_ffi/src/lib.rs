// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

#[allow(dead_code)]
pub mod interfaces;

/// All mods end up with `_impls` impl structs defined in interface.
/// Other mods which define and impl structs should not end up with name
/// `_impls`.
// All ffi impls that without raft domain.
pub mod basic_ffi_impls;
// All ffi impls that within raft domain, but without proxy helper context.
pub mod domain_impls;
// FFI directly related with EngineStoreServerHelper.
pub mod engine_store_helper_impls;
// All ffi impls that within engine store helper context.
pub mod context_impls;

pub mod sst_reader_impls;
pub mod utils;

pub use self::{
    basic_ffi_impls::*, context_impls::*, domain_impls::*, engine_store_helper_impls::*,
    interfaces::root::DB as interfaces_ffi, sst_reader_impls::*, utils::*,
};
