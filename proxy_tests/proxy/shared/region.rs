use fail::fail_point;

// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.
use crate::utils::v1::*;

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
        &mut |_, ffi: &mut FFIHelperSet| {
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
        &mut |_, ffi: &mut FFIHelperSet| {
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
            &mut |_id: u64, ffi_set: &mut FFIHelperSet| {
                let f = ffi_set.proxy_helper.fn_get_region_local_state.unwrap();
                let mut state = kvproto::raft_serverpb::RegionLocalState::default();
                let mut error_msg = mock_engine_store::RawCppStringPtrGuard::default();

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
                    .get_value_cf("none_cf", "123".as_bytes(), &mut |value: Result<Option<&[u8]>, String>| {
                        let msg = value.unwrap_err();
                        assert_eq!(msg, "Storage Engine Status { code: IoError, sub_code: None, sev: NoError, state: \"cf none_cf not found\" }");
                    });
                ffi_set
                    .proxy
                    .get_value_cf("raft", "123".as_bytes(), &mut |value: Result<
                        Option<&[u8]>,
                        String,
                    >| {
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


#[test]
fn test_stale_peer() {
    let (mut cluster, pd_client) = new_mock_cluster(0, 2);

    cluster.cfg.raft_store.max_leader_missing_duration = ReadableDuration::secs(6);

    pd_client.disable_default_operator();
    disable_auto_gen_compact_log(&mut cluster);
    // Otherwise will panic with `assert_eq!(apply_state, last_applied_state)`.
    fail::cfg("on_pre_write_apply_state", "return(true)").unwrap();
    // Set region and peers
    let r1 = cluster.run_conf_change();

    let p2 = new_learner_peer(2, 2);
    pd_client.must_add_peer(r1, p2.clone());
    cluster.must_put(b"k0", b"v");
    check_key(&cluster, b"k0", b"v", Some(true), None, Some(vec![1, 2]));


    cluster.add_send_filter(CloneFilterFactory(
        RegionPacketFilter::new(1, 2).direction(Direction::Both),
    ));

    cluster.must_put(b"k1", b"v");
    cluster.must_put(b"k2", b"v");
    check_key(&cluster, b"k2", b"v", Some(true), None, Some(vec![1]));
    check_key(&cluster, b"k2", b"v", Some(false), None, Some(vec![2]));

    pd_client.must_remove_peer(1, p2.clone());
    std::thread::sleep(std::time::Duration::from_millis(1500));

    fail::cfg("proxy_record_all_messages", "return(1)");
    cluster.clear_send_filters();

    std::thread::sleep(std::time::Duration::from_millis(1500));
    check_key(&cluster, b"k2", b"v", Some(false), None, Some(vec![2]));
    std::thread::sleep(std::time::Duration::from_millis(6000));
    assert_ne!(cluster.get_debug_struct().gc_message_count.as_ref().load(Ordering::SeqCst), 0);
}