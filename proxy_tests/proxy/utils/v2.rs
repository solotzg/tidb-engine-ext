// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

pub use proxy_test_raftstore_v2::{Cluster, NodeCluster, Simulator};

pub use super::common::*;

// pub fn new_mock_cluster(id: u64, count: usize) -> (Cluster<NodeCluster>,
// Arc<TestPdClient>) {     tikv_util::set_panic_hook(true, "./");
//     let pd_client = Arc::new(TestPdClient::new(0, false));
//     let sim = Arc::new(RwLock::new(NodeCluster::new(pd_client.clone())));
//     let mut cluster = Cluster::new(id, count, sim, pd_client.clone(),
// ProxyConfig::default());     // Compat new proxy
//     cluster.cfg.mock_cfg.proxy_compat = true;

//     #[cfg(feature = "enable-pagestorage")]
//     {
//         cluster.cfg.proxy_cfg.engine_store.enable_unips = true;
//     }
//     #[cfg(not(feature = "enable-pagestorage"))]
//     {
//         cluster.cfg.proxy_cfg.engine_store.enable_unips = false;
//     }
//     (cluster, pd_client)
// }
