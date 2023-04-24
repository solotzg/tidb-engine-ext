// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.
use std::sync::Mutex;

use test_raftstore_v2::Simulator as S2;

use super::utils::*;
use crate::utils::v1::*;

#[test]
fn test_write_simple() {
    let mut cluster_v2 = test_raftstore_v2::new_node_cluster(1, 2);
    let (mut cluster_v1, pd_client_v1) = new_mock_cluster(1, 2);
    cluster_v1
        .cfg
        .server
        .labels
        .insert(String::from("engine"), String::from("tiflash"));
    cluster_v1.pd_client.disable_default_operator();
    cluster_v2.pd_client.disable_default_operator();
    let r11 = cluster_v1.run_conf_change();
    let r21 = cluster_v2.run_conf_change();

    pd_client_v1.must_add_peer(r11, new_learner_peer(2, 10));
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
    // check_key(&cluster_v1, b"k1", b"v1", Some(true), None, Some(vec![2]));

    cluster_v1.shutdown();
    cluster_v2.shutdown();
}

// #[test]
// fn test_2_write_simple() {
//     let mut cluster_v2 = test_raftstore_v2::new_node_cluster(1, 2);
//     let cluster_v1= test_raftstore::new_node_cluster(1, 2);
//     let pd_client_v1 = cluster_v1.pd_client.clone();
//     cluster_v1
//         .cfg
//         .server
//         .labels
//         .insert(String::from("engine"), String::from("tiflash"));
//     cluster_v1.pd_client.disable_default_operator();
//     cluster_v2.pd_client.disable_default_operator();
//     let r11 = cluster_v1.run_conf_change();
//     let r21 = cluster_v2.run_conf_change();

//     let trans1 =
// Mutex::new(cluster_v1.sim.read().unwrap().get_router(2).unwrap());
//     let trans2 =
// Mutex::new(cluster_v2.sim.read().unwrap().get_router(1).unwrap());

//     let factory1 = ForwardFactory {
//         node_id: 1,
//         chain_send: Arc::new(move |m| {
//             info!("send to trans2"; "msg" => ?m);
//             let _ = trans2.lock().unwrap().send_raft_message(Box::new(m));
//         }),
//     };
//     cluster_v1.add_send_filter(factory1);
//     let factory2 = ForwardFactory {
//         node_id: 2,
//         chain_send: Arc::new(move |m| {
//             info!("send to trans1"; "msg" => ?m);
//             let _ = trans1.lock().unwrap().send_raft_message(m);
//         }),
//     };
//     cluster_v2.add_send_filter(factory2);

//     pd_client_v1
//         .must_add_peer(r11, new_learner_peer(2, 10));
//     cluster_v2
//         .pd_client
//         .must_add_peer(r21, new_learner_peer(2, 10));

//     cluster_v2.must_put(b"k1", b"v1");

//     cluster_v1.shutdown();
//     cluster_v2.shutdown();
// }
