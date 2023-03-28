// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.
#![allow(dead_code)]
#![allow(unused_variables)]

pub use mock_engine_store::mock_cluster::v1::{
    node::NodeCluster,
    transport_simulate::{
        CloneFilterFactory, CollectSnapshotFilter, Direction, RegionPacketFilter,
    },
    Cluster, Simulator,
};

pub use super::common::*;

pub fn new_mock_cluster(id: u64, count: usize) -> (Cluster<NodeCluster>, Arc<TestPdClient>) {
    tikv_util::set_panic_hook(true, "./");
    let pd_client = Arc::new(TestPdClient::new(0, false));
    let sim = Arc::new(RwLock::new(NodeCluster::new(pd_client.clone())));
    let mut cluster = Cluster::new(id, count, sim, pd_client.clone(), ProxyConfig::default());
    // Compat new proxy
    cluster.cfg.mock_cfg.proxy_compat = true;

    #[cfg(feature = "enable-pagestorage")]
    {
        cluster.cfg.proxy_cfg.engine_store.enable_unips = true;
    }
    #[cfg(not(feature = "enable-pagestorage"))]
    {
        cluster.cfg.proxy_cfg.engine_store.enable_unips = false;
    }
    (cluster, pd_client)
}

pub fn new_mock_cluster_snap(id: u64, count: usize) -> (Cluster<NodeCluster>, Arc<TestPdClient>) {
    let pd_client = Arc::new(TestPdClient::new(0, false));
    let sim = Arc::new(RwLock::new(NodeCluster::new(pd_client.clone())));
    let mut proxy_config = ProxyConfig::default();
    proxy_config.raft_store.snap_handle_pool_size = 2;
    let mut cluster = Cluster::new(id, count, sim, pd_client.clone(), proxy_config);
    // Compat new proxy
    cluster.cfg.mock_cfg.proxy_compat = true;

    #[cfg(feature = "enable-pagestorage")]
    {
        cluster.cfg.proxy_cfg.engine_store.enable_unips = true;
    }
    #[cfg(not(feature = "enable-pagestorage"))]
    {
        cluster.cfg.proxy_cfg.engine_store.enable_unips = false;
    }
    (cluster, pd_client)
}
