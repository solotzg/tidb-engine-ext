// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

pub use engine_store_ffi::TiFlashEngine;
pub use proxy_server::config::ProxyConfig;

// TODO align with engine_test when they are stable.
// Will be refered be mock_store.
// pub type ProxyRaftEngine = raft_log_engine::RaftLogEngine;
pub type ProxyRaftEngine = engine_rocks::RocksEngine;
