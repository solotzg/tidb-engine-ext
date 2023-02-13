// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use crate::RocksEngine;
use proxy_ffi::{interfaces_ffi, gen_engine_store_server_helper};
impl RocksEngine {
    pub fn get_store_stats(&self) -> interfaces_ffi::StoreStats {
        let helper = gen_engine_store_server_helper(self.engine_store_server_helper);
        helper.handle_compute_store_stats()
    }
}