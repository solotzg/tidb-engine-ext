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

    // Try to start this node, return after persisted some keys.
    let result = cluster.start();

    let k = b"k1";
    let v = b"v1";
    cluster.must_put(k, v);
    println!("!!!! After put");
    for id in cluster.engines.keys() {
        println!("!!!! After check node_id is {}", id);
        let kv = &cluster.engines[&id].kv;
        // let option = IterOptions::default();
        // let iter = kv.iterator_cf_opt("default", option);
        // for i in iter{
        //     println!("!!!! kv iter {:?} {:?}", i.key(), i.value());
        // }

        let r = kv.seek("k1".as_bytes()).unwrap().unwrap();
        println!("!!!! test_normal kv get {:?} {:?}", r.0, r.1);
        let r2 = kv.seek("LLLL".as_bytes()).unwrap().unwrap();
        println!("!!!! test_normal kv get2 {:?} {:?}", r2.0, r2.1);
        let r3 = kv.get_value_cf("default", "k1".as_bytes());
        println!("!!!! test_normal kv get3 {:?}", r3.unwrap().unwrap());
        let db: &Arc<DB> = &kv.db;
        let r4 = db.c().get_value_cf("default", "k1".as_bytes());
        println!("!!!! test_normal kv get4 {:?}", r4.unwrap().unwrap());

        // must_get_equal(&cluster.get_engine(*id), k, v);
        must_get_equal(db, k, v);
    }

    cluster.shutdown();
}
