// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use engine_tiflash::FFIHubInner;
use proxy_ffi::interfaces_ffi::EngineStoreServerHelper;
pub struct TiFlashFFIHub {
    pub engine_store_server_helper: &'static EngineStoreServerHelper,
}
unsafe impl Send for TiFlashFFIHub {}
unsafe impl Sync for TiFlashFFIHub {}

impl FFIHubInner for TiFlashFFIHub {}
