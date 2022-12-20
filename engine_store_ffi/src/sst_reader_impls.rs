// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.
pub use crate::interfaces::root::DB::{BaseBuffView, ColumnFamilyType};
use crate::{
    LockCFFileReader, RaftStoreProxyPtr, RawVoidPtr, SSTFileReader, SSTReaderPtr, SSTView,
};

impl SSTReaderPtr {
    unsafe fn as_mut_lock(&mut self) -> &mut LockCFFileReader {
        &mut *(self.inner as *mut LockCFFileReader)
    }

    unsafe fn as_mut(&mut self) -> &mut SSTFileReader {
        &mut *(self.inner as *mut SSTFileReader)
    }
}

impl From<RawVoidPtr> for SSTReaderPtr {
    fn from(pre: RawVoidPtr) -> Self {
        Self { inner: pre }
    }
}

pub unsafe extern "C" fn ffi_make_sst_reader(
    view: SSTView,
    proxy_ptr: RaftStoreProxyPtr,
) -> SSTReaderPtr {
    let path = std::str::from_utf8_unchecked(view.path.to_slice());
    let key_manager = &proxy_ptr.as_ref().key_manager;
    match view.type_ {
        ColumnFamilyType::Lock => {
            LockCFFileReader::ffi_get_cf_file_reader(path, key_manager.as_ref()).into()
        }
        _ => SSTFileReader::ffi_get_cf_file_reader(path, key_manager.clone()).into(),
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
