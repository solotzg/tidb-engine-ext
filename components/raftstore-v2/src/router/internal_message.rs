// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use kvproto::raft_serverpb::RegionLocalState;

use crate::operation::CommittedEntries;

#[derive(Debug)]
pub enum ApplyTask {
    CommittedEntries(CommittedEntries),
}

#[derive(Debug, Default)]
pub struct ApplyRes {
    pub applied_index: u64,
    pub applied_term: u64,
    pub region_state: Option<RegionLocalState>,
}
