// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use std::sync::{atomic::AtomicU8, Arc};

use encryption::DataKeyManager;
use engine_store_ffi::ffi::interfaces_ffi::{
    EngineStoreServerHelper, RaftProxyStatus, RaftStoreProxyFFIHelper,
};
use engine_traits::Engines;
use raftstore::store::RaftRouter;
use tikv_util::sys::SysQuota;

use crate::{
    config::MockConfig, mock_cluster::TikvConfig, mock_store::gen_engine_store_server_helper,
    EngineStoreServer, EngineStoreServerWrap, TiFlashEngine,
};

pub struct EngineHelperSet {
    pub engine_store_server: Box<EngineStoreServer>,
    pub engine_store_server_wrap: Box<EngineStoreServerWrap>,
    pub engine_store_server_helper: Box<EngineStoreServerHelper>,
}

pub struct FFIHelperSet {
    pub proxy: Box<engine_store_ffi::ffi::RaftStoreProxy>,
    pub proxy_helper: Box<RaftStoreProxyFFIHelper>,
    pub engine_store_server: Box<EngineStoreServer>,
    // Make interface happy, don't own proxy and server.
    pub engine_store_server_wrap: Box<EngineStoreServerWrap>,
    pub engine_store_server_helper: Box<EngineStoreServerHelper>,
    pub engine_store_server_helper_ptr: isize,
}

pub struct TestData {
    pub expected_leader_safe_ts: u64,
    pub expected_self_safe_ts: u64,
}

pub struct ClusterExt {}

impl ClusterExt {
    pub fn make_ffi_helper_set_no_bind(
        id: u64,
        engines: Engines<TiFlashEngine, engine_rocks::RocksEngine>,
        key_mgr: &Option<Arc<DataKeyManager>>,
        router: &Option<RaftRouter<TiFlashEngine, engine_rocks::RocksEngine>>,
        node_cfg: TikvConfig,
        cluster_id: isize,
        proxy_compat: bool,
        mock_cfg: MockConfig,
    ) -> (FFIHelperSet, TikvConfig) {
        // We must allocate on heap to avoid move.
        let proxy = Box::new(engine_store_ffi::ffi::RaftStoreProxy::new(
            AtomicU8::new(RaftProxyStatus::Idle as u8),
            key_mgr.clone(),
            match router {
                Some(r) => Some(Box::new(
                    engine_store_ffi::ffi::read_index_helper::ReadIndexClient::new(
                        r.clone(),
                        SysQuota::cpu_cores_quota() as usize * 2,
                    ),
                )),
                None => None,
            },
            engine_store_ffi::ffi::RaftStoreProxyEngine::from_tiflash_engine(engines.kv.clone()),
        ));

        let proxy_ref = proxy.as_ref();
        let mut proxy_helper = Box::new(RaftStoreProxyFFIHelper::new(proxy_ref.into()));
        let mut engine_store_server = Box::new(EngineStoreServer::new(id, Some(engines)));
        engine_store_server.proxy_compat = proxy_compat;
        engine_store_server.mock_cfg = mock_cfg;
        let engine_store_server_wrap = Box::new(EngineStoreServerWrap::new(
            &mut *engine_store_server,
            Some(&mut *proxy_helper),
            cluster_id,
        ));
        let engine_store_server_helper = Box::new(gen_engine_store_server_helper(
            std::pin::Pin::new(&*engine_store_server_wrap),
        ));

        let engine_store_server_helper_ptr = &*engine_store_server_helper as *const _ as isize;
        proxy
            .kv_engine()
            .write()
            .unwrap()
            .as_mut()
            .unwrap()
            .set_engine_store_server_helper(engine_store_server_helper_ptr);
        let ffi_helper_set = FFIHelperSet {
            proxy,
            proxy_helper,
            engine_store_server,
            engine_store_server_wrap,
            engine_store_server_helper,
            engine_store_server_helper_ptr,
        };
        (ffi_helper_set, node_cfg)
    }
}
