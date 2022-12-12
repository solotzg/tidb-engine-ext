// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

#![feature(slice_take)]
pub mod config;
pub mod mock_cluster;
pub mod mock_store;
pub mod node;
pub mod server;
pub mod transport_simulate;

pub use mock_store::*;

pub fn copy_meta_from<EK: engine_traits::KvEngine, ER: RaftEngine + engine_traits::Peekable>(
    source_engines: &Engines<EK, ER>,
    target_engines: &Engines<EK, ER>,
    source: &Region,
    target: &mut Region,
    new_region_meta: kvproto::metapb::Region,
    copy_region_state: bool,
    copy_apply_state: bool,
    copy_raft_state: bool,
) -> raftstore::Result<()> {
    let region_id = source.region.get_id();

    let mut wb = target_engines.kv.write_batch();

    // Can't copy this key, otherwise will cause a bootstrap.
    // box_try!(wb.put_msg(keys::PREPARE_BOOTSTRAP_KEY, &source.region));

    // region local state
    if copy_region_state {
        let mut state = RegionLocalState::default();
        state.set_region(new_region_meta);
        box_try!(wb.put_msg_cf(CF_RAFT, &keys::region_state_key(region_id), &state));
    }

    // apply state
    if copy_apply_state {
        let apply_state: RaftApplyState =
            match general_get_apply_state(&source_engines.kv, region_id) {
                Some(x) => x,
                None => return Err(box_err!("bad RaftApplyState")),
            };
        wb.put_msg_cf(CF_RAFT, &keys::apply_state_key(region_id), &apply_state)?;
        target.apply_state = apply_state.clone();
        target.applied_term = source.applied_term;
    }

    wb.write()?;
    target_engines.sync_kv()?;

    let mut raft_wb = target_engines.raft.log_batch(1024);
    // raft state
    if copy_raft_state {
        let raft_state = match get_raft_local_state(&source_engines.raft, region_id) {
            Some(x) => x,
            None => return Err(box_err!("bad RaftLocalState")),
        };
        raft_wb.put_raft_state(region_id, &raft_state)?;
    };

    box_try!(target_engines.raft.consume(&mut raft_wb, true));
    Ok(())
}

pub fn copy_data_from(
    source_engines: &Engines<impl KvEngine, impl RaftEngine + engine_traits::Peekable>,
    target_engines: &Engines<impl KvEngine, impl RaftEngine>,
    source: &Region,
    target: &mut Region,
) -> raftstore::Result<()> {
    let region_id = source.region.get_id();

    // kv data in memory
    for cf in 0..3 {
        for (k, v) in &source.data[cf] {
            write_kv_in_mem(target, cf, k.as_slice(), v.as_slice());
        }
    }

    // raft log
    let mut raft_wb = target_engines.raft.log_batch(1024);
    let mut entries: Vec<raft::eraftpb::Entry> = Default::default();
    source_engines
        .raft
        .get_all_entries_to(region_id, &mut entries)
        .unwrap();
    debug!("copy raft log {:?}", entries);

    raft_wb.append(region_id, entries)?;
    box_try!(target_engines.raft.consume(&mut raft_wb, true));
    Ok(())
}

// TODO Need refactor if moved to raft-engine
pub fn general_get_region_local_state<EK: engine_traits::KvEngine>(
    engine: &EK,
    region_id: u64,
) -> Option<RegionLocalState> {
    let region_state_key = keys::region_state_key(region_id);
    engine
        .get_msg_cf::<RegionLocalState>(CF_RAFT, &region_state_key)
        .unwrap_or(None)
}

// TODO Need refactor if moved to raft-engine
pub fn general_get_apply_state<EK: engine_traits::KvEngine>(
    engine: &EK,
    region_id: u64,
) -> Option<RaftApplyState> {
    let apply_state_key = keys::apply_state_key(region_id);
    engine
        .get_msg_cf::<RaftApplyState>(CF_RAFT, &apply_state_key)
        .unwrap_or(None)
}

pub fn get_region_local_state(
    engine: &engine_rocks::RocksEngine,
    region_id: u64,
) -> Option<RegionLocalState> {
    general_get_region_local_state(engine, region_id)
}

// TODO Need refactor if moved to raft-engine
pub fn get_apply_state(
    engine: &engine_rocks::RocksEngine,
    region_id: u64,
) -> Option<RaftApplyState> {
    general_get_apply_state(engine, region_id)
}

pub fn get_raft_local_state<ER: engine_traits::RaftEngine>(
    raft_engine: &ER,
    region_id: u64,
) -> Option<RaftLocalState> {
    match raft_engine.get_raft_state(region_id) {
        Ok(Some(x)) => Some(x),
        _ => None,
    }
}

unsafe extern "C" fn ffi_debug_func(
    arg1: *mut ffi_interfaces::EngineStoreServerWrap,
    _debug_type: u64,
    _ptr: ffi_interfaces::RawVoidPtr,
) -> ffi_interfaces::RawVoidPtr {
    std::ptr::null_mut()
}

unsafe fn create_cpp_str(s: Option<Vec<u8>>) -> ffi_interfaces::CppStrWithView {
    match s {
        Some(s) => {
            let len = s.len() as u64;
            let ptr = Box::into_raw(Box::new(s.clone())); // leak
            let s = ffi_interfaces::CppStrWithView {
                inner: ffi_interfaces::RawCppPtr {
                    ptr: ptr as RawVoidPtr,
                    type_: RawCppPtrTypeImpl::String.into(),
                },
                view: ffi_interfaces::BaseBuffView {
                    data: (*ptr).as_ptr() as *const _,
                    len,
                },
            };
            s
        }
        None => ffi_interfaces::CppStrWithView {
            inner: ffi_interfaces::RawCppPtr {
                ptr: std::ptr::null_mut(),
                type_: RawCppPtrTypeImpl::None.into(),
            },
            view: ffi_interfaces::BaseBuffView {
                data: std::ptr::null(),
                len: 0,
            },
        },
    }
}

macro_rules! unwrap_or_return {
    ($e:expr, $res:expr) => {
        match $e {
            Some(x) => x,
            None => return $res,
        }
    };
}

unsafe extern "C" fn ffi_fast_add_peer(
    arg1: *mut ffi_interfaces::EngineStoreServerWrap,
    region_id: u64,
    new_peer_id: u64,
) -> ffi_interfaces::FastAddPeerRes {
    let store = into_engine_store_server_wrap(arg1);
    let cluster = &*(store.cluster_ptr as *const mock_cluster::Cluster<NodeCluster>);
    let store_id = (*store.engine_store_server).id;

    let failed_add_peer_res =
        |status: ffi_interfaces::FastAddPeerStatus| ffi_interfaces::FastAddPeerRes {
            status,
            apply_state: create_cpp_str(None),
            region: create_cpp_str(None),
        };
    let from_store = (|| {
        fail::fail_point!("ffi_fast_add_peer_from_id", |t| {
            let t = t.unwrap().parse::<u64>().unwrap();
            t
        });
        1
    })();
    let block_wait: bool = (|| {
        fail::fail_point!("ffi_fast_add_peer_block_wait", |t| {
            let t = t.unwrap().parse::<u64>().unwrap();
            t
        });
        0
    })() != 0;
    debug!("recover from remote peer: enter from {} to {}", from_store, store_id; "region_id" => region_id);

    for retry in 0..300 {
        if retry > 0 {
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        let lock = cluster.ffi_helper_set.lock();
        let mut guard = match lock {
            Ok(e) => e,
            Err(_) => {
                error!("ffi_debug_func failed to lock");
                return failed_add_peer_res(ffi_interfaces::FastAddPeerStatus::OtherError);
            }
        };
        debug!("recover from remote peer: preparing from {} to {}, persist and check source", from_store, store_id; "region_id" => region_id);
        let source_server = match guard.get_mut(&from_store) {
            Some(s) => &mut s.engine_store_server,
            None => {
                return failed_add_peer_res(ffi_interfaces::FastAddPeerStatus::NoSuitable);
            }
        };
        let source_engines = match source_server.engines.clone() {
            Some(s) => s,
            None => {
                error!("recover from remote peer: failed get source engine"; "region_id" => region_id);
                return failed_add_peer_res(ffi_interfaces::FastAddPeerStatus::BadData);
            }
        };

        // TODO We must ask the remote peer to persist before get a snapshot.
        // {
        //     if let Some(s) = source_server.kvstore.get_mut(&region_id) {
        //         write_to_db_data_by_engine(0, &source_engines.kv, s, "fast add
        // peer".to_string());     } else {
        //         error!("recover from remote peer: failed persist source region";
        // "region_id" => region_id);         return ffi_interfaces::FastAddPeerRes
        // {             status: ffi_interfaces::FastAddPeerStatus::BadData,
        //             apply_state: create_cpp_str(None),
        //             region: create_cpp_str(None),
        //         };
        //     }
        // }
        let source_region = match source_server.kvstore.get(&region_id) {
            Some(s) => s,
            None => {
                error!("recover from remote peer: failed read source region info"; "region_id" => region_id);
                return failed_add_peer_res(ffi_interfaces::FastAddPeerStatus::BadData);
            }
        };
        let region_local_state: RegionLocalState = match general_get_region_local_state(
            &source_engines.kv,
            region_id,
        ) {
            Some(x) => x,
            None => {
                debug!("recover from remote peer: preparing from {} to {}, not region state {}", from_store, store_id, new_peer_id; "region_id" => region_id);
                // We don't return BadData here, since the data may not be persisted.
                if block_wait {
                    continue;
                }
                return failed_add_peer_res(ffi_interfaces::FastAddPeerStatus::WaitForData);
            }
        };
        let new_region_meta = region_local_state.get_region();

        if !engine_store_ffi::observer::validate_remote_peer_region(
            new_region_meta,
            store_id,
            new_peer_id,
        ) {
            debug!("recover from remote peer: preparing from {} to {}, not applied conf change {}", from_store, store_id, new_peer_id; "region_id" => region_id);
            if block_wait {
                continue;
            }
            return failed_add_peer_res(ffi_interfaces::FastAddPeerStatus::WaitForData);
        }

        debug!("recover from remote peer: preparing from {} to {}, check target", from_store, store_id; "region_id" => region_id);
        let new_region = make_new_region(
            Some(new_region_meta.clone()),
            Some((*store.engine_store_server).id),
        );
        (*store.engine_store_server)
            .kvstore
            .insert(region_id, Box::new(new_region));
        let target_engines = match (*store.engine_store_server).engines.clone() {
            Some(s) => s,
            None => {
                return failed_add_peer_res(ffi_interfaces::FastAddPeerStatus::OtherError);
            }
        };
        let target_region = match (*store.engine_store_server).kvstore.get_mut(&region_id) {
            Some(s) => s,
            None => {
                return failed_add_peer_res(ffi_interfaces::FastAddPeerStatus::BadData);
            }
        };
        debug!("recover from remote peer: meta from {} to {}", from_store, store_id; "region_id" => region_id);
        // Must first dump meta then data, otherwise data may lag behind.
        // We can see a raft log hole at applied_index otherwise.
        let apply_state: RaftApplyState = match general_get_apply_state(
            &source_engines.kv,
            region_id,
        ) {
            Some(x) => x,
            None => {
                error!("recover from remote peer: failed read apply state"; "region_id" => region_id);
                return failed_add_peer_res(ffi_interfaces::FastAddPeerStatus::BadData);
            }
        };

        debug!("recover from remote peer: data from {} to {}", from_store, store_id; "region_id" => region_id);
        if let Err(e) = copy_data_from(
            &source_engines,
            &target_engines,
            &source_region,
            target_region,
        ) {
            error!("recover from remote peer: inject error {:?}", e; "region_id" => region_id);
            return failed_add_peer_res(ffi_interfaces::FastAddPeerStatus::FailedInject);
        }

        let apply_state_bytes = apply_state.write_to_bytes().unwrap();
        let region_bytes = region_local_state.get_region().write_to_bytes().unwrap();
        let apply_state_ptr = create_cpp_str(Some(apply_state_bytes));
        let region_ptr = create_cpp_str(Some(region_bytes));
        debug!("recover from remote peer: ok from {} to {}", from_store, store_id; "region_id" => region_id);
        return ffi_interfaces::FastAddPeerRes {
            status: ffi_interfaces::FastAddPeerStatus::Ok,
            apply_state: apply_state_ptr,
            region: region_ptr,
        };
    }
    error!("recover from remote peer: failed after retry"; "region_id" => region_id);
    return failed_add_peer_res(ffi_interfaces::FastAddPeerStatus::BadData);
}

use engine_store_ffi::RawVoidPtr;
use engine_traits::{KvEngine, Mutable, RaftEngine, RaftLogBatch, WriteBatch};
use kvproto::raft_serverpb::RaftLocalState;
use tikv_util::{box_err, box_try};

// TODO Need refactor if moved to raft-engine
pub fn general_get_region_local_state<EK: engine_traits::KvEngine>(
    engine: &EK,
    region_id: u64,
) -> Option<RegionLocalState> {
    let region_state_key = keys::region_state_key(region_id);
    engine
        .get_msg_cf::<RegionLocalState>(CF_RAFT, &region_state_key)
        .unwrap_or(None)
}

// TODO Need refactor if moved to raft-engine
pub fn general_get_apply_state<EK: engine_traits::KvEngine>(
    engine: &EK,
    region_id: u64,
) -> Option<RaftApplyState> {
    let apply_state_key = keys::apply_state_key(region_id);
    engine
        .get_msg_cf::<RaftApplyState>(CF_RAFT, &apply_state_key)
        .unwrap_or(None)
}

pub fn get_region_local_state(
    engine: &engine_rocks::RocksEngine,
    region_id: u64,
) -> Option<RegionLocalState> {
    general_get_region_local_state(engine, region_id)
}

// TODO Need refactor if moved to raft-engine
pub fn get_apply_state(
    engine: &engine_rocks::RocksEngine,
    region_id: u64,
) -> Option<RaftApplyState> {
    general_get_apply_state(engine, region_id)
}

pub fn get_raft_local_state<ER: engine_traits::RaftEngine>(
    raft_engine: &ER,
    region_id: u64,
) -> Option<RaftLocalState> {
    match raft_engine.get_raft_state(region_id) {
        Ok(Some(x)) => Some(x),
        _ => None,
    }
}

pub fn copy_meta_from<EK: engine_traits::KvEngine, ER: RaftEngine + engine_traits::Peekable>(
    source_engines: &Engines<EK, ER>,
    target_engines: &Engines<EK, ER>,
    source: &Box<Region>,
    target: &mut Box<Region>,
    new_region_meta: kvproto::metapb::Region,
    copy_region_state: bool,
    copy_apply_state: bool,
    copy_raft_state: bool,
) -> raftstore::Result<()> {
    let region_id = source.region.get_id();

    let mut wb = target_engines.kv.write_batch();

    // Can't copy this key, otherwise will cause a bootstrap.
    // box_try!(wb.put_msg(keys::PREPARE_BOOTSTRAP_KEY, &source.region));

    // region local state
    if copy_region_state {
        let mut state = RegionLocalState::default();
        state.set_region(new_region_meta);
        box_try!(wb.put_msg_cf(CF_RAFT, &keys::region_state_key(region_id), &state));
    }

    // apply state
    if copy_apply_state {
        let apply_state: RaftApplyState =
            match general_get_apply_state(&source_engines.kv, region_id) {
                Some(x) => x,
                None => return Err(box_err!("bad RaftApplyState")),
            };
        wb.put_msg_cf(CF_RAFT, &keys::apply_state_key(region_id), &apply_state)?;
        target.apply_state = apply_state.clone();
        target.applied_term = source.applied_term;
    }

    wb.write()?;
    target_engines.sync_kv()?;

    let mut raft_wb = target_engines.raft.log_batch(1024);
    // raft state
    if copy_raft_state {
        let raft_state = match get_raft_local_state(&source_engines.raft, region_id) {
            Some(x) => x,
            None => return Err(box_err!("bad RaftLocalState")),
        };
        raft_wb.put_raft_state(region_id, &raft_state)?;
    };

    box_try!(target_engines.raft.consume(&mut raft_wb, true));
    Ok(())
}

pub fn copy_data_from(
    source_engines: &Engines<impl KvEngine, impl RaftEngine + engine_traits::Peekable>,
    target_engines: &Engines<impl KvEngine, impl RaftEngine>,
    source: &Box<Region>,
    target: &mut Box<Region>,
) -> raftstore::Result<()> {
    let region_id = source.region.get_id();

    // kv data in memory
    for cf in 0..3 {
        for (k, v) in &source.data[cf] {
            write_kv_in_mem(target, cf, k.as_slice(), v.as_slice());
        }
    }

    // raft log
    let mut raft_wb = target_engines.raft.log_batch(1024);
    let mut entries: Vec<raft::eraftpb::Entry> = Default::default();
    source_engines
        .raft
        .get_all_entries_to(region_id, &mut entries)
        .unwrap();
    debug!("copy raft log {:?}", entries);

    raft_wb.append(region_id, entries)?;
    box_try!(target_engines.raft.consume(&mut raft_wb, true));
    Ok(())
}
