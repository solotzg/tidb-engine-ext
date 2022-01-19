// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::sync::{Arc, RwLock};

use engine_traits::{IterOptions, Iterable, Iterator, Peekable};
use kvproto::{metapb, raft_serverpb};
use mock_engine_store;
use test_raftstore::*;
#[test]
fn test_normal() {
    let pd_client = Arc::new(TestPdClient::new(0, false));
    let sim = Arc::new(RwLock::new(NodeCluster::new(pd_client.clone())));
    let mut cluster = Cluster::new(0, 3, sim, pd_client);
    unsafe {
        test_raftstore::init_cluster_ptr(&cluster);
    }

    // Try to start this node, return after persisted some keys.
    let _ = cluster.start();

    let k = b"k1";
    let v = b"v1";
    cluster.must_put(k, v);
    for id in cluster.engines.keys() {
        must_get_equal(&cluster.get_engine(*id), k, v);
        // must_get_equal(db, k, v);
    }
    let region_id = cluster.get_region(k).get_id();
    unsafe {
        for (k, ffi_set) in cluster.ffi_helper_set.iter() {
            let f = ffi_set.proxy_helper.fn_get_region_peer_state.unwrap();
            let mut r = 0;
            assert_eq!(
                f(ffi_set.proxy_helper.proxy_ptr.clone(), region_id, &mut r),
                1
            );
            assert_eq!(r, 0);
        }
    }

    cluster.shutdown();
}
