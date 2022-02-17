// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use pd_client::PdClient;
use raft::eraftpb::MessageType;
use test_raftstore::*;

fn get_addr(pd_client: &std::sync::Arc<TestPdClient>, node_id: u64) -> String {
    if cfg!(feature = "test-raftstore-proxy") {
        pd_client
            .get_store(node_id)
            .unwrap()
            .get_peer_address()
            .to_string()
    } else {
        pd_client
            .get_store(node_id)
            .unwrap()
            .get_address()
            .to_string()
    }
}

/// When encountering raft/batch_raft mismatch store id error, the service is expected
/// to drop connections in order to let raft_client re-resolve store address from PD
/// This will make the mismatch error be automatically corrected.
/// Ths test verified this case.
#[test]
fn test_mismatch_store_node() {
    let count = 3;
    let mut cluster = new_server_cluster(0, count);
    cluster.run();
    cluster.must_put(b"k1", b"v1");
    let node_ids = cluster.get_node_ids();
    let mut iter = node_ids.iter();
    let node1_id = *iter.next().unwrap();
    let node2_id = *iter.next().unwrap();
    let node3_id = *iter.next().unwrap();
    let pd_client = cluster.pd_client.clone();
    must_get_equal(&cluster.get_engine(node1_id), b"k1", b"v1");
    must_get_equal(&cluster.get_engine(node2_id), b"k1", b"v1");
    must_get_equal(&cluster.get_engine(node3_id), b"k1", b"v1");
    let node1_addr = get_addr(&pd_client, node1_id);
    let node2_addr = get_addr(&pd_client, node2_id);
    let node3_addr = get_addr(&pd_client, node3_id);
    cluster.stop_node(node2_id);
    cluster.stop_node(node3_id);
    // run node2
    cluster.cfg.server.addr = node3_addr.clone();
    cluster.run_node(node2_id).unwrap();
    let filter = RegionPacketFilter::new(1, node2_id)
        .direction(Direction::Send)
        .msg_type(MessageType::MsgRequestPreVote);
    cluster.add_send_filter(CloneFilterFactory(filter));
    // run node3
    cluster.cfg.server.addr = node2_addr.clone();
    cluster.run_node(node3_id).unwrap();
    let filter = RegionPacketFilter::new(1, node3_id)
        .direction(Direction::Send)
        .msg_type(MessageType::MsgRequestPreVote);
    cluster.add_send_filter(CloneFilterFactory(filter));
    sleep_ms(600);
    fail::cfg("mock_store_refresh_interval_secs", "return(0)").unwrap();
    cluster.must_put(b"k2", b"v2");
    assert_eq!(node1_addr, get_addr(&pd_client, node1_id));
    assert_eq!(node3_addr, get_addr(&pd_client, node2_id));
    assert_eq!(node2_addr, get_addr(&pd_client, node3_id));
    must_get_equal(&cluster.get_engine(node3_id), b"k2", b"v2");
    must_get_equal(&cluster.get_engine(node2_id), b"k2", b"v2");
    fail::remove("mock_store_refresh_interval_secs");
}
