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
        let tikv_key = keys::data_key(k);
        println!("!!!! Check engine node_id is {}", id);
        let kv = &cluster.engines[&id].kv;
        let db: &Arc<DB> = &kv.db;
        // let r = kv.seek(&[122, 1]).unwrap().unwrap();
        // println!("!!!! test_normal kv get {:?}", r.0);
        // let r3 = kv.get_value_cf("default", &tikv_key);
        // println!("!!!! test_normal kv get3 {:?}", r3.unwrap().unwrap());
        let r4 = db.c().get_value_cf("default", &tikv_key);
        println!("!!!! test_normal kv get4 overall {:?}", r4);
        match r4 {
            Ok(v) => {
                if v.is_some() {
                    println!("!!!! test_normal kv get4 {:?}", v.unwrap());
                } else {
                    println!("!!!! test_normal kv get4 is None");
                }
            }
            Err(e) => println!("!!!! test_normal kv get4 is Error"),
        }

        // must_get_equal(&cluster.get_engine(*id), k, v);
        // must_get_equal(db, k, v);
    }

    cluster.shutdown();
}
