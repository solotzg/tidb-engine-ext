// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::sync::{Arc, RwLock};

use engine_traits::Peekable;
use kvproto::{metapb, raft_serverpb};
use mock_engine_store;
use std::sync::atomic::{AtomicBool, AtomicU8};
use test_raftstore::*;

#[test]
fn test_normal() {
    let pd_client = Arc::new(TestPdClient::new(0, false));
    let sim = Arc::new(RwLock::new(NodeCluster::new(pd_client.clone())));
    let mut cluster = Cluster::new(0, 3, sim, pd_client);

    // Try to start this node, return after persisted some keys.
    let result = cluster.start();

    let k = b"k1";
    let v = b"v1";
    cluster.must_put(k, v);
    // println!("!!!! After put");
    // for id in cluster.engines.keys() {
    //     println!("!!!! After check eq {}", id);
    //     must_get_equal(&cluster.get_engine(*id), k, v);
    // }

    cluster.shutdown();
}
