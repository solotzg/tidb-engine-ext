use std::iter::FromIterator;

use collections::HashSet;

// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.
use crate::proxy::*;

#[test]
fn test_handle_destroy() {
    let (mut cluster, pd_client) = new_mock_cluster(0, 3);

    disable_auto_gen_compact_log(&mut cluster);

    // Disable default max peer count check.
    pd_client.disable_default_operator();

    cluster.run();
    cluster.must_put(b"k1", b"v1");
    let eng_ids = cluster
        .engines
        .iter()
        .map(|e| e.0.to_owned())
        .collect::<Vec<_>>();

    let region = cluster.get_region(b"k1");
    let region_id = region.get_id();
    let peer_1 = find_peer(&region, eng_ids[0]).cloned().unwrap();
    let peer_2 = find_peer(&region, eng_ids[1]).cloned().unwrap();
    cluster.must_transfer_leader(region_id, peer_1);

    iter_ffi_helpers(
        &cluster,
        Some(vec![eng_ids[1]]),
        &mut |_, _, ffi: &mut FFIHelperSet| {
            let server = &ffi.engine_store_server;
            assert!(server.kvstore.contains_key(&region_id));
        },
    );

    pd_client.must_remove_peer(region_id, peer_2);

    check_key(
        &cluster,
        b"k1",
        b"v2",
        Some(false),
        None,
        Some(vec![eng_ids[1]]),
    );

    std::thread::sleep(std::time::Duration::from_millis(100));
    // Region removed in server.
    iter_ffi_helpers(
        &cluster,
        Some(vec![eng_ids[1]]),
        &mut |_, _, ffi: &mut FFIHelperSet| {
            let server = &ffi.engine_store_server;
            assert!(!server.kvstore.contains_key(&region_id));
        },
    );

    cluster.shutdown();
}

#[test]
fn test_get_region_local_state() {
    let (mut cluster, _pd_client) = new_mock_cluster(0, 3);

    cluster.run();

    let k = b"k1";
    let v = b"v1";
    cluster.must_put(k, v);
    check_key(&cluster, k, v, Some(true), None, None);
    let region_id = cluster.get_region(k).get_id();

    // Get RegionLocalState through ffi
    unsafe {
        iter_ffi_helpers(
            &cluster,
            None,
            &mut |_id: u64, _, ffi_set: &mut FFIHelperSet| {
                let f = ffi_set.proxy_helper.fn_get_region_local_state.unwrap();
                let mut state = kvproto::raft_serverpb::RegionLocalState::default();
                let mut error_msg = new_mock_engine_store::RawCppStringPtrGuard::default();

                assert_eq!(
                    f(
                        ffi_set.proxy_helper.proxy_ptr,
                        region_id,
                        &mut state as *mut _ as _,
                        error_msg.as_mut(),
                    ),
                    KVGetStatus::Ok
                );
                assert!(state.has_region());
                assert_eq!(state.get_state(), kvproto::raft_serverpb::PeerState::Normal);
                assert!(error_msg.as_ref().is_null());

                let mut state = kvproto::raft_serverpb::RegionLocalState::default();
                assert_eq!(
                    f(
                        ffi_set.proxy_helper.proxy_ptr,
                        0, // not exist
                        &mut state as *mut _ as _,
                        error_msg.as_mut(),
                    ),
                    KVGetStatus::NotFound
                );
                assert!(!state.has_region());
                assert!(error_msg.as_ref().is_null());

                ffi_set
                    .proxy
                    .get_value_cf("none_cf", "123".as_bytes(), |value| {
                        let msg = value.unwrap_err();
                        assert_eq!(msg, "Storage Engine Status { code: IoError, sub_code: None, sev: NoError, state: \"cf none_cf not found\" }");
                    });
                ffi_set
                    .proxy
                    .get_value_cf("raft", "123".as_bytes(), |value| {
                        let res = value.unwrap();
                        assert!(res.is_none());
                    });

                // If we have no kv engine.
                ffi_set.proxy.set_kv_engine(None);
                let res = ffi_set.proxy_helper.fn_get_region_local_state.unwrap()(
                    ffi_set.proxy_helper.proxy_ptr,
                    region_id,
                    &mut state as *mut _ as _,
                    error_msg.as_mut(),
                );
                assert_eq!(res, KVGetStatus::Error);
                assert!(!error_msg.as_ref().is_null());
                assert_eq!(
                    error_msg.as_str(),
                    "KV engine is not initialized".as_bytes()
                );
            },
        );
    }

    cluster.shutdown();
}

/// This test is very important.
/// If make sure we can add learner peer for a store which is not started
/// actually.
#[test]
fn test_add_invalid_learner_peer_by_simple() {
    let (mut cluster, pd_client) = new_mock_cluster(0, 3);
    disable_auto_gen_compact_log(&mut cluster);
    // Disable default max peer count check.
    pd_client.disable_default_operator();

    let _ = cluster.run();
    cluster.must_put(b"k1", b"v1");
    check_key(&cluster, b"k1", b"v1", Some(true), None, Some(vec![1]));

    pd_client.must_add_peer(1, new_learner_peer(4, 4));

    cluster.must_put(b"k3", b"v3");
    check_key(&cluster, b"k3", b"v3", Some(true), None, None);
    let new_states = collect_all_states(&cluster, 1);
    assert_eq!(new_states.len(), 3);
    for i in new_states.keys() {
        assert_eq!(
            new_states
                .get(i)
                .unwrap()
                .in_disk_region_state
                .get_region()
                .get_peers()
                .len(),
            3 + 1 // Learner
        );
    }

    cluster.shutdown();
}

/// This test is very important.
/// If make sure we can add learner peer for a store which is not started
/// actually.
#[test]
fn test_add_invalid_learner_peer_by_joint() {
    let (mut cluster, pd_client) = new_mock_cluster(0, 3);
    disable_auto_gen_compact_log(&mut cluster);
    // Disable default max peer count check.
    pd_client.disable_default_operator();

    let _ = cluster.run_conf_change();
    cluster.must_put(b"k1", b"v1");
    check_key(&cluster, b"k1", b"v1", Some(true), None, Some(vec![1]));

    pd_client.must_joint_confchange(
        1,
        vec![
            (ConfChangeType::AddNode, new_peer(2, 2)),
            (ConfChangeType::AddNode, new_peer(3, 3)),
            (ConfChangeType::AddLearnerNode, new_learner_peer(4, 4)),
            (ConfChangeType::AddLearnerNode, new_learner_peer(5, 5)),
        ],
    );
    assert!(pd_client.is_in_joint(1));
    pd_client.must_leave_joint(1);

    cluster.must_put(b"k3", b"v3");
    check_key(&cluster, b"k3", b"v3", Some(true), None, None);
    let new_states = collect_all_states(&cluster, 1);
    assert_eq!(new_states.len(), 3);
    for i in new_states.keys() {
        assert_eq!(
            new_states
                .get(i)
                .unwrap()
                .in_disk_region_state
                .get_region()
                .get_peers()
                .len(),
            1 + 2 /* AddPeer */ + 2 // Learner
        );
    }

    cluster.shutdown();
}

use engine_traits::{Engines, KvEngine, RaftEngine};
use raftstore::store::{write_initial_apply_state, write_initial_raft_state};

pub fn prepare_bootstrap_cluster_with(
    engines: &Engines<impl KvEngine, impl RaftEngine>,
    region: &metapb::Region,
) -> raftstore::Result<()> {
    let mut state = RegionLocalState::default();
    state.set_region(region.clone());

    let mut wb = engines.kv.write_batch();
    box_try!(wb.put_msg(keys::PREPARE_BOOTSTRAP_KEY, region));
    box_try!(wb.put_msg_cf(CF_RAFT, &keys::region_state_key(region.get_id()), &state));
    write_initial_apply_state(&mut wb, region.get_id())?;
    wb.write()?;
    engines.sync_kv()?;

    let mut raft_wb = engines.raft.log_batch(1024);
    write_initial_raft_state(&mut raft_wb, region.get_id())?;
    box_try!(engines.raft.consume(&mut raft_wb, true));
    Ok(())
}

/// This test is very important.
/// If make sure we can add learner peer for a store which is not started
/// actually.
#[test]
fn test_add_learner_peer_before_start_by_joint() {
    let (mut cluster, pd_client) = new_mock_cluster(0, 5);
    fail::cfg("on_pre_persist_with_finish", "return").unwrap();
    cluster.cfg.proxy_compat = false;
    disable_auto_gen_compact_log(&mut cluster);
    // Disable default max peer count check.
    pd_client.disable_default_operator();

    let _ = cluster.run_conf_change_no_start();
    let _ = cluster.start_with(HashSet::from_iter(
        vec![3, 4].into_iter().map(|x| x as usize),
    ));
    cluster.must_put(b"k1", b"v1");
    check_key(&cluster, b"k1", b"v1", Some(true), None, Some(vec![1]));

    pd_client.must_joint_confchange(
        1,
        vec![
            (ConfChangeType::AddNode, new_peer(2, 2)),
            (ConfChangeType::AddNode, new_peer(3, 3)),
            (ConfChangeType::AddLearnerNode, new_learner_peer(4, 4)),
            (ConfChangeType::AddLearnerNode, new_learner_peer(5, 5)),
        ],
    );
    assert!(pd_client.is_in_joint(1));
    pd_client.must_leave_joint(1);

    cluster.must_put(b"k3", b"v3");
    check_key(
        &cluster,
        b"k3",
        b"v3",
        Some(true),
        None,
        Some(vec![1, 2, 3]),
    );

    let new_states = maybe_collect_states(&cluster, 1, Some(vec![1, 2, 3]));
    assert_eq!(new_states.len(), 3);
    for i in new_states.keys() {
        assert_eq!(
            new_states
                .get(i)
                .unwrap()
                .in_disk_region_state
                .get_region()
                .get_peers()
                .len(),
            1 + 2 /* AddPeer */ + 2 // Learner
        );
    }

    let region = new_states
        .get(&1)
        .unwrap()
        .in_disk_region_state
        .get_region();
    assert_eq!(cluster.ffi_helper_lst.len(), 2);

    for id in vec![4, 5] {
        let engines = cluster.get_engines(id);
        assert!(prepare_bootstrap_cluster_with(engines, region).is_ok());
    }

    cluster
        .start_with(HashSet::from_iter(
            vec![0, 1, 2].into_iter().map(|x| x as usize),
        ))
        .unwrap();

    // pd_client.must_joint_confchange(
    //     1,
    //     vec![
    //         (ConfChangeType::AddNode, new_peer(2, 2)),
    //         (ConfChangeType::AddNode, new_peer(3, 3)),
    //         (ConfChangeType::AddLearnerNode, new_learner_peer(4, 4)),
    //         (ConfChangeType::AddLearnerNode, new_learner_peer(5, 5)),
    //     ],
    // );
    // // assert!(pd_client.is_in_joint(1));
    // pd_client.must_leave_joint(1);

    cluster.must_put(b"k4", b"v4");
    // std::thread::sleep(std::time::Duration::from_millis(3000));
    check_key(&cluster, b"k4", b"v4", Some(true), None, None);

    let new_states = maybe_collect_states(&cluster, 1, None);
    assert_eq!(new_states.len(), 5);
    for i in new_states.keys() {
        assert_eq!(
            new_states
                .get(i)
                .unwrap()
                .in_disk_region_state
                .get_region()
                .get_peers()
                .len(),
            1 + 2 /* AddPeer */ + 2 // Learner
        );
    }

    fail::remove("on_pre_persist_with_finish");
    cluster.shutdown();
}
