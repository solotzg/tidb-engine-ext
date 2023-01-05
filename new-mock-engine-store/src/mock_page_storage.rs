// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use std::{collections::btree_map::OccupiedEntry, sync::RwLock};

use collections::HashMap;
pub use engine_store_ffi::{
    interfaces::root::DB as ffi_interfaces, BaseBuffView, CppStrWithView, EngineStoreServerHelper,
    PageAndCppStrWithView, PageAndCppStrWithViewVec, PageWithView, RaftStoreProxyFFIHelper,
    RawCppPtr, RawVoidPtr,
};

use crate::mock_store::{into_engine_store_server_wrap, EngineStoreServerWrap, RawCppPtrTypeImpl};

#[derive(Default)]
pub struct MockPSWriteBatch {
    pub data: HashMap<Vec<u8>, MockPSUniversalPage>,
}

pub struct MockPSUniversalPage {
    data: Vec<u8>,
}

impl Into<MockPSUniversalPage> for BaseBuffView {
    fn into(self) -> MockPSUniversalPage {
        MockPSUniversalPage {
            data: self.to_slice().to_owned(),
        }
    }
}

#[derive(Default)]
pub struct MockPageStorage {
    pub data: RwLock<HashMap<Vec<u8>, MockPSUniversalPage>>,
}

pub unsafe extern "C" fn ffi_mockps_create_write_batch() -> RawCppPtr {
    let ptr = Box::into_raw(Box::new(MockPSWriteBatch::default()));
    RawCppPtr {
        ptr: ptr as RawVoidPtr,
        type_: RawCppPtrTypeImpl::PSWriteBatch.into(),
    }
}

impl From<RawVoidPtr> for &mut MockPSWriteBatch {
    fn from(value: RawVoidPtr) -> Self {
        unsafe { &mut *(value as *mut MockPSWriteBatch) }
    }
}

pub unsafe extern "C" fn ffi_mockps_write_batch_put_page(
    wb: RawVoidPtr,
    page_id: BaseBuffView,
    page: BaseBuffView,
) {
    let wb: _ = <&mut MockPSWriteBatch as From<RawVoidPtr>>::from(wb);
    wb.data.insert(page_id.to_slice().to_owned(), page.into());
}

pub unsafe extern "C" fn ffi_mockps_write_batch_del_page(wb: RawVoidPtr, page_id: BaseBuffView) {
    let wb: _ = <&mut MockPSWriteBatch as From<RawVoidPtr>>::from(wb);
    wb.data.remove(page_id.to_slice());
}

pub unsafe extern "C" fn ffi_mockps_write_batch_size(wb: RawVoidPtr) -> u64 {
    let wb: _ = <&mut MockPSWriteBatch as From<RawVoidPtr>>::from(wb);
    wb.data.len() as u64
}

pub unsafe extern "C" fn ffi_mockps_write_batch_is_empty(wb: RawVoidPtr) -> u8 {
    let wb: _ = <&mut MockPSWriteBatch as From<RawVoidPtr>>::from(wb);
    if wb.data.is_empty() { 1 } else { 0 }
}

pub unsafe extern "C" fn ffi_mockps_write_batch_merge(lwb: RawVoidPtr, rwb: RawVoidPtr) {
    let lwb: _ = <&mut MockPSWriteBatch as From<RawVoidPtr>>::from(lwb);
    let rwb: _ = <&mut MockPSWriteBatch as From<RawVoidPtr>>::from(rwb);
    lwb.data.extend(rwb.data.drain());
}

pub unsafe extern "C" fn ffi_mockps_write_batch_clear(wb: RawVoidPtr) {
    let wb: _ = <&mut MockPSWriteBatch as From<RawVoidPtr>>::from(wb);
    wb.data.clear();
}

pub unsafe extern "C" fn ffi_mockps_consume_write_batch(
    wrap: *const ffi_interfaces::EngineStoreServerWrap,
    wb: RawVoidPtr,
) {
    let store = into_engine_store_server_wrap(wrap);
    let wb: _ = <&mut MockPSWriteBatch as From<RawVoidPtr>>::from(wb);
    let mut guard = (*store.engine_store_server)
        .page_storage
        .data
        .write()
        .unwrap();
    guard.extend(wb.data.drain());
}

pub unsafe extern "C" fn ffi_mockps_handle_read_page(
    wrap: *const ffi_interfaces::EngineStoreServerWrap,
    page_id: BaseBuffView,
) -> PageWithView {
    todo!()
}

pub unsafe extern "C" fn ffi_mockps_handle_scan_page(
    wrap: *const ffi_interfaces::EngineStoreServerWrap,
    start_page_id: BaseBuffView,
    end_page_id: BaseBuffView,
) -> PageAndCppStrWithViewVec {
    todo!()
}

pub unsafe extern "C" fn ffi_mockps_handle_purge_pagestorage(
    wrap: *const ffi_interfaces::EngineStoreServerWrap,
) {
    todo!()
}

pub unsafe extern "C" fn ffi_mockps_handle_seek_ps_key(
    wrap: *const ffi_interfaces::EngineStoreServerWrap,
    page_id: BaseBuffView,
) -> CppStrWithView {
    todo!()
}

pub unsafe extern "C" fn ffi_mockps_ps_is_empty(
    wrap: *const ffi_interfaces::EngineStoreServerWrap,
) -> u8 {
    let store = into_engine_store_server_wrap(wrap);
    let guard = (*store.engine_store_server)
        .page_storage
        .data
        .read()
        .unwrap();
    if guard.is_empty() { 1 } else { 0 }
}
