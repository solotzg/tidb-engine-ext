// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

use engine_traits::KvEngine;
use raftstore::store::fsm::{ApplyRouter, ApplyTask};

pub trait ApplyRouterHelper: Sync + Send {
    fn schedule_compact_log_task(
        &self,
        region_id: u64,
        compact_index: u64,
        compact_term: u64,
        applied_index: u64,
    );
}

pub struct ProxyApplyRouterHelper<EK: KvEngine> {
    pub apply_router: std::sync::Mutex<ApplyRouter<EK>>,
}

impl<EK: KvEngine> ProxyApplyRouterHelper<EK> {
    pub fn new(apply_router: ApplyRouter<EK>) -> Self {
        Self {
            apply_router: std::sync::Mutex::new(apply_router.clone()),
        }
    }
}

impl<EK: KvEngine> ApplyRouterHelper for ProxyApplyRouterHelper<EK> {
    fn schedule_compact_log_task(
        &self,
        region_id: u64,
        compact_index: u64,
        compact_term: u64,
        applied_index: u64,
    ) {
        self.apply_router.lock().unwrap().schedule_task(
            region_id,
            ApplyTask::CheckCompact {
                region_id,
                voter_replicated_index: compact_index,
                voter_replicated_term: compact_term,
                applied_index: Some(applied_index),
            },
        )
    }
}
