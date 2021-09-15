// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::sync::{Arc, RwLock};

use engine_rocks::raw::DB;
use engine_rocks::{Compat, RocksEngine, RocksSnapshot};
use engine_traits::IterOptions;
use engine_traits::Iterable;
use engine_traits::{Iterator, Peekable};
use kvproto::{metapb, raft_serverpb};
use mock_engine_store;
use std::sync::atomic::{AtomicBool, AtomicU8};
use test_raftstore::*;
#[test]
fn test_normal() {
    let pd_client = Arc::new(TestPdClient::new(0, false));
    let sim = Arc::new(RwLock::new(NodeCluster::new(pd_client.clone())));
    let mut cluster = Cluster::new(0, 3, sim, pd_client);
    unsafe {
        test_raftstore::init_cluster_ptr(&cluster);
    }

    cluster.make_global_ffi_helper_set();
    // Try to start this node, return after persisted some keys.
    let result = cluster.start();

    let k = b"k1";
    let v = b"v1";
    cluster.must_put(k, v);
    println!("!!!! After put");
    test_raftstore::print_all_cluster(std::str::from_utf8(k).unwrap());
    for id in cluster.engines.keys() {
        must_get_equal(&cluster.get_engine(*id), k, v);
        // must_get_equal(db, k, v);
    }

    cluster.shutdown();
}
