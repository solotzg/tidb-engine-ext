// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::{
    collections::HashMap,
    io::{self, Read, Write},
    ops::{Deref, DerefMut},
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc, Once, RwLock,
    },
};

use engine_traits::{
    Error, ExternalSstFileInfo, Iterable, Iterator, MiscExt, Mutable, Peekable, Result, SeekKey,
    SstExt, SstReader, SstWriter, SstWriterBuilder, WriteBatch, WriteBatchExt, CF_DEFAULT, CF_LOCK,
    CF_RAFT, CF_WRITE,
};
use kvproto::{
    raft_cmdpb::{AdminCmdType, AdminRequest},
    raft_serverpb::{RaftApplyState, RegionLocalState, StoreIdent},
};
use new_mock_engine_store::{
    mock_cluster::FFIHelperSet,
    node::NodeCluster,
    transport_simulate::{
        CloneFilterFactory, CollectSnapshotFilter, Direction, RegionPacketFilter,
    },
    Cluster, ProxyConfig, Simulator, TestPdClient,
};
use pd_client::PdClient;
use proxy_server::{
    config::{address_proxy_config, ensure_no_common_unrecognized_keys},
    run::run_tikv_proxy,
};
use raft::eraftpb::MessageType;
use raftstore::{
    coprocessor::{ConsistencyCheckMethod, Coprocessor},
    engine_store_ffi,
    engine_store_ffi::{KVGetStatus, RaftStoreProxyFFI},
    store::util::find_peer,
};
use server::setup::validate_and_persist_config;
use sst_importer::SstImporter;
use test_raftstore::new_tikv_config;
pub use test_raftstore::{must_get_equal, must_get_none, new_peer};
use tikv::config::TiKvConfig;
use tikv_util::{
    config::{LogFormat, ReadableDuration, ReadableSize},
    time::Duration,
    HandyRwLock,
};

use crate::proxy::*;

#[test]
fn test_config() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    let text = "memory-usage-high-water=0.65\nsnap-handle-pool-size=4\n[nosense]\nfoo=2\n[rocksdb]\nmax-open-files = 111\nz=1";
    write!(file, "{}", text).unwrap();
    let path = file.path();

    let mut unrecognized_keys = Vec::new();
    let mut config = TiKvConfig::from_file(path, Some(&mut unrecognized_keys)).unwrap();
    assert_eq!(config.memory_usage_high_water, 0.65);
    assert_eq!(config.rocksdb.max_open_files, 111);
    assert_eq!(unrecognized_keys.len(), 3);

    let mut proxy_unrecognized_keys = Vec::new();
    let proxy_config = ProxyConfig::from_file(path, Some(&mut proxy_unrecognized_keys)).unwrap();
    assert_eq!(proxy_config.snap_handle_pool_size, 4);
    let v1 = vec!["a.b", "b"]
        .iter()
        .map(|e| String::from(*e))
        .collect::<Vec<String>>();
    let v2 = vec!["a.b", "b.b", "c"]
        .iter()
        .map(|e| String::from(*e))
        .collect::<Vec<String>>();
    let unknown = ensure_no_common_unrecognized_keys(&v1, &v2);
    assert_eq!(unknown.is_err(), true);
    assert_eq!(unknown.unwrap_err(), "a.b, b.b");
    let unknown = ensure_no_common_unrecognized_keys(&proxy_unrecognized_keys, &unrecognized_keys);
    assert_eq!(unknown.is_err(), true);
    assert_eq!(unknown.unwrap_err(), "nosense, rocksdb.z");

    // Need run this test with ENGINE_LABEL_VALUE=tiflash, otherwise will fatal exit.
    server::setup::validate_and_persist_config(&mut config, true);

    // Will not override ProxyConfig
    let proxy_config_new = ProxyConfig::from_file(path, None).unwrap();
    assert_eq!(proxy_config_new.snap_handle_pool_size, 4);
}

#[test]
fn test_store_setup() {
    let (mut cluster, pd_client) = new_mock_cluster(0, 3);

    // Add label to cluster
    address_proxy_config(&mut cluster.cfg.tikv);

    // Try to start this node, return after persisted some keys.
    let _ = cluster.start();
    let store_id = cluster.engines.keys().last().unwrap();
    let store = pd_client.get_store(*store_id).unwrap();
    println!("store {:?}", store);
    assert!(
        store
            .get_labels()
            .iter()
            .find(|&x| x.key == "engine" && x.value == "tiflash")
            .is_some()
    );

    cluster.shutdown();
}

#[test]
fn test_consistency_check() {
    // ComputeHash and VerifyHash shall be filtered.
    let (mut cluster, pd_client) = new_mock_cluster(0, 2);

    cluster.run();

    cluster.must_put(b"k", b"v");
    let region = cluster.get_region("k".as_bytes());
    let region_id = region.get_id();

    let r = new_verify_hash_request(vec![1, 2, 3, 4, 5, 6], 1000);
    let req = test_raftstore::new_admin_request(region_id, region.get_region_epoch(), r);
    let res = cluster
        .call_command_on_leader(req, Duration::from_secs(3))
        .unwrap();

    let r = new_verify_hash_request(vec![7, 8, 9, 0], 1000);
    let req = test_raftstore::new_admin_request(region_id, region.get_region_epoch(), r);
    let res = cluster
        .call_command_on_leader(req, Duration::from_secs(3))
        .unwrap();

    cluster.must_put(b"k2", b"v2");
    cluster.shutdown();
}

#[test]
fn test_old_compact_log() {
    // TODO If we just return None for CompactLog, the region state in ApplyFsm will change.
    // because, there are no rollback in new implementation.
    let (mut cluster, pd_client) = new_mock_cluster(0, 3);
    cluster.run();

    // We don't return Persist after handling CompactLog.
    fail::cfg("no_persist_compact_log", "return").unwrap();
    for i in 0..10 {
        let k = format!("k{}", i);
        let v = format!("v{}", i);
        cluster.must_put(k.as_bytes(), v.as_bytes());
    }

    for i in 0..10 {
        let k = format!("k{}", i);
        let v = format!("v{}", i);
        check_key(&cluster, k.as_bytes(), v.as_bytes(), Some(true), None, None);
    }

    let region = cluster.get_region(b"k1");
    let region_id = region.get_id();
    let prev_state = collect_all_states(&cluster, region_id);
    let (compact_index, compact_term) = get_valid_compact_index(&prev_state);
    let compact_log = test_raftstore::new_compact_log_request(compact_index, compact_term);
    let req = test_raftstore::new_admin_request(region_id, region.get_region_epoch(), compact_log);
    let res = cluster
        .call_command_on_leader(req, Duration::from_secs(3))
        .unwrap();

    cluster.shutdown();
}

#[test]
fn test_compact_log() {
    let (mut cluster, pd_client) = new_mock_cluster(0, 3);
    cluster.run();

    cluster.must_put(b"k", b"v");
    let region = cluster.get_region("k".as_bytes());
    let region_id = region.get_id();

    fail::cfg("on_empty_cmd_normal", "return").unwrap();
    fail::cfg("try_flush_data", "return(0)").unwrap();
    for i in 0..10 {
        let k = format!("k{}", i);
        let v = format!("v{}", i);
        cluster.must_put(k.as_bytes(), v.as_bytes());
    }

    let prev_state = collect_all_states(&cluster, region_id);

    let (compact_index, compact_term) = get_valid_compact_index(&prev_state);
    let compact_log = test_raftstore::new_compact_log_request(compact_index, compact_term);
    let req = test_raftstore::new_admin_request(region_id, region.get_region_epoch(), compact_log);
    let res = cluster
        .call_command_on_leader(req, Duration::from_secs(3))
        .unwrap();
    // compact index should less than applied index
    assert!(!res.get_header().has_error(), "{:?}", res);

    // CompactLog is filtered, because we can't flush data.
    let new_state = collect_all_states(&cluster, region_id);
    for i in prev_state.keys() {
        let old = prev_state.get(i).unwrap();
        let new = new_state.get(i).unwrap();
        assert_eq!(
            old.in_memory_apply_state.get_truncated_state(),
            new.in_memory_apply_state.get_truncated_state()
        );
        assert_eq!(
            old.in_disk_apply_state.get_truncated_state(),
            new.in_disk_apply_state.get_truncated_state()
        );
    }

    fail::remove("on_empty_cmd_normal");
    fail::remove("try_flush_data");

    let (compact_index, compact_term) = get_valid_compact_index(&new_state);
    let compact_log = test_raftstore::new_compact_log_request(compact_index, compact_term);
    let req = test_raftstore::new_admin_request(region_id, region.get_region_epoch(), compact_log);
    let res = cluster
        .call_command_on_leader(req, Duration::from_secs(3))
        .unwrap();
    assert!(!res.get_header().has_error(), "{:?}", res);

    cluster.must_put(b"kz", b"vz");
    check_key(&cluster, b"kz", b"vz", Some(true), None, None);

    // CompactLog is not filtered
    let new_state = collect_all_states(&cluster, region_id);
    for i in prev_state.keys() {
        let old = prev_state.get(i).unwrap();
        let new = new_state.get(i).unwrap();
        assert_ne!(
            old.in_memory_apply_state.get_truncated_state(),
            new.in_memory_apply_state.get_truncated_state()
        );
    }

    cluster.shutdown();
}

#[test]
fn test_get_region_local_state() {
    let (mut cluster, pd_client) = new_mock_cluster(0, 3);

    cluster.start().unwrap();

    let k = b"k1";
    let v = b"v1";
    cluster.must_put(k, v);
    for id in cluster.engines.keys() {
        must_get_equal(&cluster.get_engine(*id), k, v);
    }
    let region_id = cluster.get_region(k).get_id();

    // Get RegionLocalState through ffi
    unsafe {
        iter_ffi_helpers(
            &cluster,
            None,
            &mut |id: u64, _, ffi_set: &mut FFIHelperSet| {
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
                    .get_value_cf("none_cf", "123".as_bytes(), |value| {
                        let msg = value.unwrap_err();
                        assert_eq!(msg, "Storage Engine cf none_cf not found");
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
