// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.
//! This mod overrides common in TiKV.

use std::{
    cmp,
    convert::TryFrom,
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Duration,
    u64,
};

use api_version::{dispatch_api_version, KvFormat};
use concurrency_manager::ConcurrencyManager;
use encryption_export::{data_key_manager_from_config, DataKeyManager};
use engine_rocks::{
    flush_engine_statistics, from_rocks_compression_type,
    raw::{Cache, Env},
    RocksEngine, RocksStatistics,
};
use engine_rocks_helper::sst_recovery::{RecoveryRunner, DEFAULT_CHECK_INTERVAL};
use engine_store_ffi::{
    self,
    core::DebugStruct,
    ffi::{
        interfaces_ffi::{
            EngineStoreServerHelper, EngineStoreServerStatus, RaftProxyStatus,
            RaftStoreProxyFFIHelper,
        },
        read_index_helper::ReadIndexClient,
        RaftStoreProxy, RaftStoreProxyFFI,
    },
    TiFlashEngine,
};
use engine_tiflash::PSLogEngine;
use engine_traits::{
    CachedTablet, CfOptionsExt, Engines, FlowControlFactorsExt, KvEngine, MiscExt, RaftEngine,
    SingletonFactory, StatisticsReporter, TabletContext, TabletRegistry, CF_DEFAULT, CF_LOCK,
    CF_WRITE,
};
use error_code::ErrorCodeExt;
use file_system::{
    get_io_rate_limiter, BytesFetcher, File, IoBudgetAdjustor, MetricsManager as IOMetricsManager,
};
use futures::executor::block_on;
use grpcio::{EnvBuilder, Environment};
use grpcio_health::HealthService;
use kvproto::{
    debugpb::create_debug, diagnosticspb::create_diagnostics, import_sstpb::create_import_sst,
};
use pd_client::{PdClient, RpcClient};
use raft_log_engine::RaftLogEngine;
use raftstore::{
    coprocessor::{config::SplitCheckConfigManager, CoprocessorHost, RegionInfoAccessor},
    router::ServerRaftStoreRouter,
    store::{
        config::RaftstoreConfigManager,
        fsm,
        fsm::store::{
            RaftBatchSystem, RaftRouter, StoreMeta, MULTI_FILES_SNAPSHOT_FEATURE, PENDING_MSG_CAP,
        },
        memory::MEMTRACE_ROOT as MEMTRACE_RAFTSTORE,
        AutoSplitController, CheckLeaderRunner, LocalReader, SnapManager, SnapManagerBuilder,
        SplitCheckRunner, SplitConfigManager, StoreMetaDelegate,
    },
};
use resource_control::{
    ResourceGroupManager, ResourceManagerService, MIN_PRIORITY_UPDATE_INTERVAL,
};
use security::SecurityManager;
use server::{memory::*, raft_engine_switch::*};
use tikv::{
    config::{ConfigController, DbConfigManger, DbType, TikvConfig},
    coprocessor::{self, MEMTRACE_ROOT as MEMTRACE_COPROCESSOR},
    coprocessor_v2,
    import::{ImportSstService, SstImporter},
    read_pool::{build_yatp_read_pool, ReadPool, ReadPoolConfigManager},
    server::{
        config::{Config as ServerConfig, ServerConfigManager},
        gc_worker::GcWorker,
        raftkv::ReplicaReadLockChecker,
        resolve,
        service::{DebugService, DiagnosticsService},
        tablet_snap::NoSnapshotCache,
        ttl::TtlChecker,
        KvEngineFactoryBuilder, Node, RaftKv, Server, CPU_CORES_QUOTA_GAUGE, DEFAULT_CLUSTER_ID,
        GRPC_THREAD_PREFIX,
    },
    storage::{
        self,
        config_manager::StorageConfigManger,
        kv::LocalTablets,
        txn::flow_controller::{EngineFlowController, FlowController},
        Engine, Storage,
    },
};
use tikv_util::{
    check_environment_variables,
    config::{ensure_dir_exist, RaftDataStateMachine, ReadableDuration, VersionTrack},
    error,
    math::MovingAvgU32,
    quota_limiter::{QuotaLimitConfigManager, QuotaLimiter},
    sys::{disk, register_memory_usage_high_water, thread::ThreadBuildWrapper, SysQuota},
    thread_group::GroupProperties,
    time::{Instant, Monitor},
    worker::{Builder as WorkerBuilder, LazyWorker, Scheduler, Worker},
    yatp_pool::CleanupMethod,
    Either,
};
use tokio::runtime::Builder;

use crate::{common::Stop, status_server::StatusServer};

impl<E: 'static, R> Stop for StatusServer<E, R>
where
    R: 'static + Send,
{
    fn stop(self: Box<Self>) {
        (*self).stop()
    }
}

pub trait ConfiguredRaftEngine: RaftEngine {
    fn build(
        _: &TikvConfig,
        _: &Arc<Env>,
        _: &Option<Arc<DataKeyManager>>,
        _: &Cache,
    ) -> (Self, Option<Arc<RocksStatistics>>);
    fn as_rocks_engine(&self) -> Option<&RocksEngine> {
        None
    }

    fn register_config(&self, _cfg_controller: &mut ConfigController) {}

    fn as_ps_engine(&mut self) -> Option<&mut PSLogEngine> {
        None
    }
}

impl ConfiguredRaftEngine for engine_rocks::RocksEngine {
    fn build(
        config: &TikvConfig,
        env: &Arc<Env>,
        key_manager: &Option<Arc<DataKeyManager>>,
        block_cache: &Cache,
    ) -> (Self, Option<Arc<RocksStatistics>>) {
        let mut raft_data_state_machine = RaftDataStateMachine::new(
            &config.storage.data_dir,
            &config.raft_engine.config().dir,
            &config.raft_store.raftdb_path,
        );
        let should_dump = raft_data_state_machine.before_open_target();

        let raft_db_path = &config.raft_store.raftdb_path;
        let config_raftdb = &config.raftdb;
        let statistics = Arc::new(RocksStatistics::new_titan());
        let raft_db_opts = config_raftdb.build_opt(env.clone(), Some(&statistics));
        let raft_cf_opts = config_raftdb.build_cf_opts(block_cache);
        let raftdb = engine_rocks::util::new_engine_opt(raft_db_path, raft_db_opts, raft_cf_opts)
            .expect("failed to open raftdb");

        if should_dump {
            let raft_engine =
                RaftLogEngine::new(config.raft_engine.config(), key_manager.clone(), None)
                    .expect("failed to open raft engine for migration");
            dump_raft_engine_to_raftdb(&raft_engine, &raftdb, 8 /* threads */);
            raft_engine.stop();
            drop(raft_engine);
            raft_data_state_machine.after_dump_data();
        }
        (raftdb, Some(statistics))
    }

    fn as_rocks_engine(&self) -> Option<&RocksEngine> {
        Some(self)
    }

    fn register_config(&self, cfg_controller: &mut ConfigController) {
        cfg_controller.register(
            tikv::config::Module::Raftdb,
            Box::new(DbConfigManger::new(self.clone(), DbType::Raft)),
        );
    }
}

impl ConfiguredRaftEngine for RaftLogEngine {
    fn build(
        config: &TikvConfig,
        env: &Arc<Env>,
        key_manager: &Option<Arc<DataKeyManager>>,
        block_cache: &Cache,
    ) -> (Self, Option<Arc<RocksStatistics>>) {
        let mut raft_data_state_machine = RaftDataStateMachine::new(
            &config.storage.data_dir,
            &config.raft_store.raftdb_path,
            &config.raft_engine.config().dir,
        );
        let should_dump = raft_data_state_machine.before_open_target();

        let raft_config = config.raft_engine.config();
        let raft_engine =
            RaftLogEngine::new(raft_config, key_manager.clone(), get_io_rate_limiter())
                .expect("failed to open raft engine");

        if should_dump {
            let config_raftdb = &config.raftdb;
            let raft_db_opts = config_raftdb.build_opt(env.clone(), None);
            let raft_cf_opts = config_raftdb.build_cf_opts(block_cache);
            let raftdb = engine_rocks::util::new_engine_opt(
                &config.raft_store.raftdb_path,
                raft_db_opts,
                raft_cf_opts,
            )
            .expect("failed to open raftdb for migration");
            dump_raftdb_to_raft_engine(&raftdb, &raft_engine, 8 /* threads */);
            raftdb.stop();
            drop(raftdb);
            raft_data_state_machine.after_dump_data();
        }
        (raft_engine, None)
    }
}

impl ConfiguredRaftEngine for PSLogEngine {
    fn build(
        _config: &TikvConfig,
        _env: &Arc<Env>,
        _key_manager: &Option<Arc<DataKeyManager>>,
        _block_cache: &Cache,
    ) -> (Self, Option<Arc<RocksStatistics>>) {
        // create a dummy file in raft engine dir to pass initial config check
        let raft_engine_path = _config.raft_engine.config().dir + "/ps_engine";
        let path = Path::new(&raft_engine_path);
        if !path.exists() {
            File::create(path).unwrap();
        }
        (PSLogEngine::new(), None)
    }

    fn as_ps_engine(&mut self) -> Option<&mut PSLogEngine> {
        Some(self)
    }
}

pub struct EnginesResourceInfo {
    kv_engine: TiFlashEngine,
    raft_engine: Option<RocksEngine>,
    latest_normalized_pending_bytes: AtomicU32,
    normalized_pending_bytes_collector: MovingAvgU32,
}

impl EnginesResourceInfo {
    const SCALE_FACTOR: u64 = 100;

    pub fn new<CER: ConfiguredRaftEngine>(
        engines: &Engines<TiFlashEngine, CER>,
        max_samples_to_preserve: usize,
    ) -> Self {
        let raft_engine = engines.raft.as_rocks_engine().cloned();
        EnginesResourceInfo {
            kv_engine: engines.kv.clone(),
            raft_engine,
            latest_normalized_pending_bytes: AtomicU32::new(0),
            normalized_pending_bytes_collector: MovingAvgU32::new(max_samples_to_preserve),
        }
    }

    pub fn update(&self, _now: Instant) {
        let mut normalized_pending_bytes = 0;

        fn fetch_engine_cf(engine: &RocksEngine, cf: &str, normalized_pending_bytes: &mut u32) {
            if let Ok(cf_opts) = engine.get_options_cf(cf) {
                if let Ok(Some(b)) = engine.get_cf_pending_compaction_bytes(cf) {
                    if cf_opts.get_soft_pending_compaction_bytes_limit() > 0 {
                        *normalized_pending_bytes = std::cmp::max(
                            *normalized_pending_bytes,
                            (b * EnginesResourceInfo::SCALE_FACTOR
                                / cf_opts.get_soft_pending_compaction_bytes_limit())
                                as u32,
                        );
                    }
                }
            }
        }

        if let Some(raft_engine) = &self.raft_engine {
            fetch_engine_cf(raft_engine, CF_DEFAULT, &mut normalized_pending_bytes);
        }
        for cf in &[CF_DEFAULT, CF_WRITE, CF_LOCK] {
            fetch_engine_cf(&self.kv_engine.rocks, cf, &mut normalized_pending_bytes);
        }
        let (_, avg) = self
            .normalized_pending_bytes_collector
            .add(normalized_pending_bytes);
        self.latest_normalized_pending_bytes.store(
            std::cmp::max(normalized_pending_bytes, avg),
            Ordering::Relaxed,
        );
    }
}

impl IoBudgetAdjustor for EnginesResourceInfo {
    fn adjust(&self, total_budgets: usize) -> usize {
        let score = self.latest_normalized_pending_bytes.load(Ordering::Relaxed) as f32
            / Self::SCALE_FACTOR as f32;
        // Two reasons for adding `sqrt` on top:
        // 1) In theory the convergence point is independent of the value of pending
        //    bytes (as long as backlog generating rate equals consuming rate, which is
        //    determined by compaction budgets), a convex helps reach that point while
        //    maintaining low level of pending bytes.
        // 2) Variance of compaction pending bytes grows with its magnitude, a filter
        //    with decreasing derivative can help balance such trend.
        let score = score.sqrt();
        // The target global write flow slides between Bandwidth / 2 and Bandwidth.
        let score = 0.5 + score / 2.0;
        (total_budgets as f32 * score) as usize
    }
}
