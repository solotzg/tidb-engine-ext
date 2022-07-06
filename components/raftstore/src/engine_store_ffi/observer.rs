use std::sync::{Arc, Mutex};

use collections::HashMap;
use engine_tiflash::FsStatsExt;

use crate::{
    engine_store_ffi::{
        interfaces::root::{DB as ffi_interfaces, DB::EngineStoreApplyRes},
        ColumnFamilyType, RaftCmdHeader, TiFlashEngine, WriteCmdType,
    },
    store::SnapKey,
};

impl Into<engine_tiflash::FsStatsExt> for ffi_interfaces::StoreStats {
    fn into(self) -> FsStatsExt {
        FsStatsExt {
            available: self.fs_stats.avail_size,
            capacity: self.fs_stats.capacity_size,
            used: self.fs_stats.used_size,
        }
    }
}

pub struct TiFlashFFIHub {
    pub engine_store_server_helper: &'static crate::engine_store_ffi::EngineStoreServerHelper,
}
unsafe impl Send for TiFlashFFIHub {}
unsafe impl Sync for TiFlashFFIHub {}

impl engine_tiflash::FFIHubInner for TiFlashFFIHub {
    fn get_store_stats(&self) -> engine_tiflash::FsStatsExt {
        self.engine_store_server_helper
            .handle_compute_store_stats()
            .into()
    }
}

pub struct PtrWrapper(crate::RawCppPtr);

unsafe impl Send for PtrWrapper {}
unsafe impl Sync for PtrWrapper {}

#[derive(Default, Debug)]
pub struct PrehandleContext {
    // tracer holds ptr of snapshot prehandled by TiFlash side.
    pub tracer: HashMap<SnapKey, Arc<PrehandleTask>>,
}
unsafe impl Send for PrehandleTask {}
unsafe impl Sync for PrehandleTask {}

pub struct TiFlashObserver {
    pub peer_id: u64,
    pub engine_store_server_helper: &'static crate::EngineStoreServerHelper,
    pub engine: TiFlashEngine,
    pub sst_importer: Arc<SSTImporter>,
    pub pre_handle_snapshot_ctx: Arc<Mutex<PrehandleContext>>,
}

impl Clone for TiFlashObserver {
    fn clone(&self) -> Self {
        TiFlashObserver {
            peer_id: self.peer_id,
            engine_store_server_helper: self.engine_store_server_helper,
            engine: self.engine.clone(),
            sst_importer: self.sst_importer.clone(),
            pre_handle_snapshot_ctx: self.pre_handle_snapshot_ctx.clone(),
        }
    }
}

// TiFlash observer's priority should be higher than all other observers, to avoid being bypassed.
const TIFLASH_OBSERVER_PRIORITY: u32 = 0;

impl TiFlashObserver {
    pub fn new(
        peer_id: u64,
        engine: engine_tiflash::RocksEngine,
        sst_importer: Arc<SSTImporter>,
    ) -> Self {
        let engine_store_server_helper =
            crate::gen_engine_store_server_helper(engine.engine_store_server_helper);
        TiFlashObserver {
            peer_id,
            engine_store_server_helper,
            engine,
            sst_importer,
            pre_handle_snapshot_ctx: Arc::new(Mutex::new(PrehandleContext::default())),
        }
    }
}
