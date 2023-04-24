// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use engine_traits::Peekable;
use kvproto::raft_serverpb::{PeerState, RaftMessage};
use raftstore::errors::Result;
use test_raftstore::{new_learner_peer, sleep_ms, Filter, FilterFactory, Simulator as S1};
use test_raftstore_v2::Simulator as S2;
use tikv_util::time::Instant;

struct ForwardFactory {
    node_id: u64,
    chain_send: Arc<dyn Fn(RaftMessage) + Send + Sync + 'static>,
}

impl FilterFactory for ForwardFactory {
    fn generate(&self, _: u64) -> Vec<Box<dyn Filter>> {
        vec![Box::new(ForwardFilter {
            node_id: self.node_id,
            chain_send: self.chain_send.clone(),
        })]
    }
}

struct ForwardFilter {
    node_id: u64,
    chain_send: Arc<dyn Fn(RaftMessage) + Send + Sync + 'static>,
}

impl Filter for ForwardFilter {
    fn before(&self, msgs: &mut Vec<RaftMessage>) -> Result<()> {
        for m in msgs.drain(..) {
            if self.node_id == m.get_to_peer().get_store_id() {
                (self.chain_send)(m);
            }
        }
        Ok(())
    }
}

#[test]
fn test_write_simple() {
    tikv_util::set_panic_hook(true, "./");
    test_util::init_log_for_test();
    let mut cluster_v2 = test_raftstore_v2::new_node_cluster(1, 2);
    let mut cluster_v1 = test_raftstore::new_node_cluster(1, 2);
    cluster_v1
        .cfg
        .server
        .labels
        .insert(String::from("engine"), String::from("tiflash"));
    cluster_v1.pd_client.disable_default_operator();
    cluster_v2.pd_client.disable_default_operator();
    let r11 = cluster_v1.run_conf_change();
    let r21 = cluster_v2.run_conf_change();

    cluster_v1
        .pd_client
        .must_add_peer(r11, new_learner_peer(2, 10));
    cluster_v2
        .pd_client
        .must_add_peer(r21, new_learner_peer(2, 10));

    let trans1 = Mutex::new(cluster_v1.sim.read().unwrap().get_router(2).unwrap());
    let trans2 = Mutex::new(cluster_v2.sim.read().unwrap().get_router(1).unwrap());

    let factory1 = ForwardFactory {
        node_id: 1,
        chain_send: Arc::new(move |m| {
            info!("send to trans2"; "msg" => ?m);
            let _ = trans2.lock().unwrap().send_raft_message(Box::new(m));
        }),
    };
    cluster_v1.add_send_filter(factory1);
    let factory2 = ForwardFactory {
        node_id: 2,
        chain_send: Arc::new(move |m| {
            info!("send to trans1"; "msg" => ?m);
            let _ = trans1.lock().unwrap().send_raft_message(m);
        }),
    };
    cluster_v2.add_send_filter(factory2);

    cluster_v2.must_put(b"k1", b"v1");
    assert_eq!(
        cluster_v2.must_get(b"k1").unwrap(),
        "v1".as_bytes().to_vec()
    );
    std::thread::sleep(std::time::Duration::from_millis(1000));
    let v = cluster_v1
        .get_engine(2)
        .get_value(b"k1")
        .unwrap()
        .unwrap()
        .to_vec();
    assert_eq!(v, "v1".as_bytes().to_vec());

    cluster_v1.shutdown();
    cluster_v2.shutdown();
}
