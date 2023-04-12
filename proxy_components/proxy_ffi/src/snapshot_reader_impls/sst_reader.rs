// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use super::{sst_file_reader::*, LockCFFileReader};
use crate::{
    interfaces_ffi::{
        BaseBuffView, ColumnFamilyType, RaftStoreProxyPtr, SSTReaderInterfaces, SSTReaderPtr,
        SSTView,
    },
    raftstore_proxy_helper_impls::RaftStoreProxyFFI,
};

#[allow(clippy::clone_on_copy)]
impl Clone for SSTReaderInterfaces {
    fn clone(&self) -> SSTReaderInterfaces {
        SSTReaderInterfaces {
            fn_get_sst_reader: self.fn_get_sst_reader.clone(),
            fn_remained: self.fn_remained.clone(),
            fn_key: self.fn_key.clone(),
            fn_value: self.fn_value.clone(),
            fn_next: self.fn_next.clone(),
            fn_gc: self.fn_gc.clone(),
        }
    }
}

impl SSTReaderPtr {
    unsafe fn as_mut_lock(&mut self) -> &mut LockCFFileReader {
        &mut *(self.inner as *mut LockCFFileReader)
    }

    unsafe fn as_mut(&mut self) -> &mut SSTFileReader {
        &mut *(self.inner as *mut SSTFileReader)
    }
}

#[allow(clippy::clone_on_copy)]
impl Clone for SSTReaderPtr {
    fn clone(&self) -> SSTReaderPtr {
        SSTReaderPtr {
            inner: self.inner.clone(),
            kind: self.kind,
        }
    }
}

#[allow(clippy::clone_on_copy)]
impl Clone for SSTView {
    fn clone(&self) -> SSTView {
        SSTView {
            type_: self.type_.clone(),
            path: self.path.clone(),
        }
    }
}

pub unsafe extern "C" fn ffi_make_sst_reader(
    view: SSTView,
    proxy_ptr: RaftStoreProxyPtr,
) -> SSTReaderPtr {
    let path = std::str::from_utf8_unchecked(view.path.to_slice());
    let key_manager = proxy_ptr.as_ref().maybe_key_manager();
    match view.type_ {
        ColumnFamilyType::Lock => {
            LockCFFileReader::ffi_get_cf_file_reader(path, key_manager.as_ref())
        }
        _ => SSTFileReader::ffi_get_cf_file_reader(path, key_manager.clone()),
    }
}

pub unsafe extern "C" fn ffi_sst_reader_remained(
    mut reader: SSTReaderPtr,
    type_: ColumnFamilyType,
) -> u8 {
    match type_ {
        ColumnFamilyType::Lock => reader.as_mut_lock().ffi_remained(),
        _ => reader.as_mut().ffi_remained(),
    }
}

pub unsafe extern "C" fn ffi_sst_reader_key(
    mut reader: SSTReaderPtr,
    type_: ColumnFamilyType,
) -> BaseBuffView {
    match type_ {
        ColumnFamilyType::Lock => reader.as_mut_lock().ffi_key(),
        _ => reader.as_mut().ffi_key(),
    }
}

pub unsafe extern "C" fn ffi_sst_reader_val(
    mut reader: SSTReaderPtr,
    type_: ColumnFamilyType,
) -> BaseBuffView {
    match type_ {
        ColumnFamilyType::Lock => reader.as_mut_lock().ffi_val(),
        _ => reader.as_mut().ffi_val(),
    }
}

pub unsafe extern "C" fn ffi_sst_reader_next(mut reader: SSTReaderPtr, type_: ColumnFamilyType) {
    match type_ {
        ColumnFamilyType::Lock => reader.as_mut_lock().ffi_next(),
        _ => reader.as_mut().ffi_next(),
    }
}

pub unsafe extern "C" fn ffi_gc_sst_reader(reader: SSTReaderPtr, type_: ColumnFamilyType) {
    match type_ {
        ColumnFamilyType::Lock => {
            drop(Box::from_raw(reader.inner as *mut LockCFFileReader));
        }
        _ => {
            drop(Box::from_raw(reader.inner as *mut SSTFileReader));
        }
    }
}

pub(crate) const KIND_SST: u64 = 0;
#[allow(unused)]
pub(crate) const KIND_TABLET: u64 = 1;
