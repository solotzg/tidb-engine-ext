// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.
use std::{collections::BTreeMap, sync::atomic::AtomicU64};

use crate::common::*;

pub type RegionId = u64;
#[derive(Default, Clone)]
pub struct MockRegion {
    pub region: kvproto::metapb::Region,
    // Which peer is me?
    pub peer: kvproto::metapb::Peer,
    // in-memory data
    pub data: [BTreeMap<Vec<u8>, Vec<u8>>; 3],
    // If we a key is deleted, it will immediately be removed from data,
    // We will record the key in pending_delete, so we can delete it from disk when flushing.
    pub pending_delete: [HashSet<Vec<u8>>; 3],
    pub pending_write: [BTreeMap<Vec<u8>, Vec<u8>>; 3],
    pub apply_state: kvproto::raft_serverpb::RaftApplyState,
    pub applied_term: u64,
}

impl MockRegion {
    pub fn set_applied(&mut self, index: u64, term: u64) {
        self.apply_state.set_applied_index(index);
        self.applied_term = term;
    }

    pub fn new(meta: kvproto::metapb::Region) -> Self {
        MockRegion {
            region: meta,
            peer: Default::default(),
            data: Default::default(),
            pending_delete: Default::default(),
            pending_write: Default::default(),
            apply_state: Default::default(),
            applied_term: 0,
        }
    }
}

#[derive(Default)]
pub struct RegionStats {
    pub pre_handle_count: AtomicU64,
    pub fast_add_peer_count: AtomicU64,
}

// In case of newly added cfs.
#[allow(unreachable_patterns)]
pub fn cf_to_name(cf: interfaces_ffi::ColumnFamilyType) -> &'static str {
    match cf {
        interfaces_ffi::ColumnFamilyType::Lock => CF_LOCK,
        interfaces_ffi::ColumnFamilyType::Write => CF_WRITE,
        interfaces_ffi::ColumnFamilyType::Default => CF_DEFAULT,
        _ => unreachable!(),
    }
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
