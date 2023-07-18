// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.
use proxy_ffi::interfaces_ffi::{RaftStoreProxyFFIHelper, RaftStoreProxyPtr};

use crate::utils::v1::*;

fn do_notify_compact_log(
    helper: &RaftStoreProxyFFIHelper,
    ptr: RaftStoreProxyPtr,
    region_id: u64,
    compact_index: u64,
    compact_term: u64,
    applied_index: u64,
) {
    unsafe {
        (helper.fn_notify_compact_log.as_ref().unwrap())(
            ptr,
            region_id,
            compact_index,
            compact_term,
            applied_index,
        )
    }
}

#[test]
fn test_proactive_flush() {
    let (mut cluster, _) = new_mock_cluster(0, 1);
    disable_auto_gen_compact_log(&mut cluster);
    fail::cfg("try_flush_data", "return(0)").unwrap();
    cluster.run();

    let region = cluster.get_region(b"k1");
    let region_id = region.get_id();
    must_put_and_check_key(&mut cluster, 1, 10, Some(true), None, None);

    let prev_state = collect_all_states(&cluster.cluster_ext, region_id);
    let (compact_index, compact_term) = get_valid_compact_index(&prev_state);
    let compact_log = test_raftstore::new_compact_log_request(compact_index, compact_term);
    let req = test_raftstore::new_admin_request(region_id, region.get_region_epoch(), compact_log);
    let _ = cluster
        .call_command_on_leader(req, Duration::from_secs(3))
        .unwrap();

    std::thread::sleep(std::time::Duration::from_secs(1));

    let new_state = collect_all_states(&cluster.cluster_ext, region_id);
    must_unaltered_disk_apply_state(&prev_state, &new_state);
    let prev_state = new_state;

    let applied_index = compact_index;
    iter_ffi_helpers(&cluster, Some(vec![1]), &mut |_, ffi: &mut FFIHelperSet| {
        do_notify_compact_log(
            ffi.proxy_helper.as_ref(),
            ffi.proxy_helper.proxy_ptr.clone(),
            region_id,
            compact_index,
            compact_term,
            applied_index,
        )
    });
    // Wait the async scheduled
    std::thread::sleep(std::time::Duration::from_secs(2));
    let new_state = collect_all_states(&cluster.cluster_ext, region_id);
    must_altered_disk_apply_state(&prev_state, &new_state);
    iter_ffi_helpers(&cluster, Some(vec![1]), &mut |_, ffi: &mut FFIHelperSet| {
        assert_eq!(
            ffi.engine_store_server
                .engines
                .as_ref()
                .unwrap()
                .kv
                .proxy_ext
                .debug_struct
                .proactive_compact_log_count
                .load(Ordering::SeqCst),
            1
        )
    });
    fail::remove("try_flush_data");
    cluster.shutdown();
}
