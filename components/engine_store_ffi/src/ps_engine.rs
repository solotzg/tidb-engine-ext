// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.
// Disable warnings for unused engine_rocks's feature.
#![allow(dead_code)]
#![allow(unused_variables)]

use std::{fmt, slice};

use std::{
    fmt::{Formatter, Debug},
    fs,
    marker::Send,
    path::Path,
    ops::Deref,
};

use engine_traits::{
    RaftEngine, RaftEngineDebug, RaftEngineReadOnly, RaftLogBatch, RaftLogGCTask, Result,
};

use protobuf::Message;
use raft::eraftpb::Entry;
use kvproto::raft_serverpb::RaftLocalState;

use tikv_util::{box_err};

use crate::{gen_engine_store_server_helper, RawCppPtr};

pub const RAFT_STATE_PAGE_ID: u64 = 0x02;
pub const RAFT_LOG_PAGE_ID_OFFSET: u64 = 0x05;

pub struct PSEngineWriteBatch {
    pub engine_store_server_helper: isize,
    pub raw_write_batch: RawCppPtr,
}

impl PSEngineWriteBatch {
    pub fn new(engine_store_server_helper: isize) -> PSEngineWriteBatch {
        let helper = gen_engine_store_server_helper(engine_store_server_helper);
        let raw_write_batch = helper.create_write_batch();
        PSEngineWriteBatch { engine_store_server_helper, raw_write_batch }
    }

    fn put_page(&mut self, ns_id: u64, page_id: u64, value: &[u8]) -> Result<()> {
        let helper = gen_engine_store_server_helper(self.engine_store_server_helper);
        helper.write_batch_put_page(self.raw_write_batch.ptr, ns_id, page_id, value.into());
        Ok(())
    }

    fn del_page(&mut self, ns_id: u64, page_id: u64) -> Result<()> {
        let helper = gen_engine_store_server_helper(self.engine_store_server_helper);
        helper.write_batch_del_page(self.raw_write_batch.ptr, ns_id, page_id);
        Ok(())
    }

    fn append_impl(
        &mut self,
        raft_group_id: u64,
        entries: &[Entry],
        mut ser_buf: Vec<u8>,
    ) -> Result<()> {
        for entry in entries {
            ser_buf.clear();
            entry.write_to_vec(&mut ser_buf).unwrap();
            self.put_page(raft_group_id, entry.get_index() + RAFT_LOG_PAGE_ID_OFFSET, &ser_buf)?;
        }
        Ok(())
    }

    fn put_msg<M: protobuf::Message>(&mut self, ns_id: u64, page_id: u64, m: &M) -> Result<()> {
        self.put_page(ns_id, page_id, &m.write_to_bytes()?)
    }

    fn data_size(&self) -> usize {
        panic!("not implemented");
    }

    fn clear(&self) {
        panic!("not implemented");
    }
}

impl RaftLogBatch for PSEngineWriteBatch {
    fn append(&mut self, raft_group_id: u64, entries: Vec<Entry>) -> Result<()> {
        if let Some(max_size) = entries.iter().map(|e| e.compute_size()).max() {
            let ser_buf = Vec::with_capacity(max_size as usize);
            return self.append_impl(raft_group_id, &entries, ser_buf);
        }
        Ok(())
    }

    fn cut_logs(&mut self, raft_group_id: u64, from: u64, to: u64) {
        for index in from..to {
            self.del_page(raft_group_id, index + RAFT_LOG_PAGE_ID_OFFSET).unwrap();
        }
    }

    fn put_raft_state(&mut self, raft_group_id: u64, state: &RaftLocalState) -> Result<()> {
        self.put_msg(raft_group_id, RAFT_STATE_PAGE_ID, state)
    }

    fn persist_size(&self) -> usize {
        0
    }

    fn is_empty(&self) -> bool {
        false
    }

    fn merge(&mut self, src: Self) -> Result<()> {
        std::panic!("not implemented")
    }
}

#[derive(Clone)]
pub struct PSEngine {
    pub engine_store_server_helper: isize,
}

impl std::fmt::Debug for PSEngine {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PSEngine")
            .field(
                "engine_store_server_helper",
                &self.engine_store_server_helper,
            )
            .finish()
    }
}

impl PSEngine {
    pub fn new() -> Self {
        PSEngine { engine_store_server_helper: 0 }
    }

    pub fn init(
        &mut self,
        engine_store_server_helper: isize,
    ) {
        self.engine_store_server_helper = engine_store_server_helper;
    }

    fn get_msg_cf<M: protobuf::Message + Default>(
        &self,
        raft_group_id: u64,
        page_id: u64,
    ) -> Result<Option<M>> {
        let helper = gen_engine_store_server_helper(self.engine_store_server_helper);
        let value = helper.read_page(raft_group_id, page_id);
        if value.view.len == 0 {
            return Ok(None);
        }

        let mut m = M::default();
        m.merge_from_bytes(unsafe { slice::from_raw_parts(value.view.data as *const u8, value.view.len as usize) })?;
        Ok(Some(m))
    }

    fn get_value(
        &self,
        raft_group_id: u64,
        page_id: u64,
    ) -> Option<Vec<u8>> {
        let helper = gen_engine_store_server_helper(self.engine_store_server_helper);
        let value = helper.read_page(raft_group_id, page_id);
        return if value.view.len == 0 {
            None
        } else {
            Some(value.view.to_slice().to_vec())
        }
    }
}

impl RaftEngineReadOnly for PSEngine {
    fn get_raft_state(&self, raft_group_id: u64) -> Result<Option<RaftLocalState>> {
        self.get_msg_cf(raft_group_id, RAFT_STATE_PAGE_ID)
    }

    fn get_entry(&self, raft_group_id: u64, index: u64) -> Result<Option<Entry>> {
        self.get_msg_cf(raft_group_id, index + RAFT_LOG_PAGE_ID_OFFSET)
    }

    fn fetch_entries_to(
        &self,
        region_id: u64,
        low: u64,
        high: u64,
        max_size: Option<usize>,
        buf: &mut Vec<Entry>,
    ) -> Result<usize> {
        let (max_size, mut total_size, mut count) = (max_size.unwrap_or(usize::MAX), 0, 0);

        for i in low..high {
            let index = i + RAFT_LOG_PAGE_ID_OFFSET;
            if total_size > 0 && total_size >= max_size {
                break;
            }
            match self.get_value(region_id, index) {
                None => {
                    continue;
                },
                Some(bytes) => {
                    let mut entry = Entry::default();
                    entry.merge_from_bytes(&bytes)?;
                    assert_eq!(entry.get_index(), index);
                    buf.push(entry);
                    total_size += bytes.len();
                    count += 1;
                }
            }
        }
        return Ok(count);
    }

    fn get_all_entries_to(&self, region_id: u64, buf: &mut Vec<Entry>) -> Result<()> {
        let mut found_entry = false;
        for i in 0..u64::MAX {
            let index = i + RAFT_LOG_PAGE_ID_OFFSET;
            match self.get_value(region_id, index) {
                None => {
                    if found_entry {
                        break;
                    } else {
                        continue;
                    }
                },
                Some(bytes) => {
                    found_entry = true;
                    let mut entry = Entry::default();
                    entry.merge_from_bytes(&bytes)?;
                    assert_eq!(entry.get_index(), index);
                    buf.push(entry);
                }
            }
        }
        Ok(())
    }
}

impl RaftEngineDebug for PSEngine {
    fn scan_entries<F>(&self, raft_group_id: u64, mut f: F) -> Result<()>
        where
            F: FnMut(&Entry) -> Result<bool>,
    {
        let mut found_entry = false;
        for i in 0..u64::MAX {
            let index = i + RAFT_LOG_PAGE_ID_OFFSET;
            match self.get_value(raft_group_id, index) {
                None => {
                    if found_entry {
                        break;
                    } else {
                        continue;
                    }
                },
                Some(bytes) => {
                    found_entry = true;
                    let mut entry = Entry::default();
                    entry.merge_from_bytes(&bytes)?;
                    assert_eq!(entry.get_index(), index);
                    f(&entry);
                }
            }
        }
        Ok(())
    }
}

impl RaftEngine for PSEngine {
    type LogBatch = PSEngineWriteBatch;

    fn log_batch(&self, capacity: usize) -> Self::LogBatch {
        PSEngineWriteBatch::new(self.engine_store_server_helper)
    }

    fn sync(&self) -> Result<()> {
        Ok(())
    }

    fn consume(&self, batch: &mut Self::LogBatch, sync_log: bool) -> Result<usize> {
        let bytes = batch.data_size();
        let helper = gen_engine_store_server_helper(self.engine_store_server_helper);
        helper.consume_write_batch(batch.raw_write_batch.ptr);
        batch.clear();
        Ok(bytes)
    }

    fn consume_and_shrink(
        &self,
        batch: &mut Self::LogBatch,
        sync_log: bool,
        max_capacity: usize,
        shrink_to: usize,
    ) -> Result<usize> {
        panic!("cannot understand what it is doing")
    }

    fn clean(
        &self,
        raft_group_id: u64,
        mut first_index: u64,
        state: &RaftLocalState,
        batch: &mut Self::LogBatch,
    ) -> Result<()> {
        batch.del_page(raft_group_id, RAFT_STATE_PAGE_ID);
        // TODO: find the first raft log index of this raft group
        if first_index <= state.last_index {
            for index in first_index..=state.last_index {
                batch.del_page(raft_group_id, index + RAFT_LOG_PAGE_ID_OFFSET);
            }
        }
        self.consume(batch, true);
        Ok(())
    }

    fn append(&self, raft_group_id: u64, entries: Vec<Entry>) -> Result<usize> {
        let mut wb = self.log_batch(0);
        if let Some(max_size) = entries.iter().map(|e| e.compute_size()).max() {
            let buf = Vec::with_capacity(max_size as usize);
            wb.append_impl(raft_group_id, &entries, buf)?;
            return self.consume(&mut wb, false)
        }
        Ok(0)
    }

    fn put_raft_state(&self, raft_group_id: u64, state: &RaftLocalState) -> Result<()> {
        let mut wb = self.log_batch(0);
        wb.put_msg(raft_group_id, RAFT_STATE_PAGE_ID, state);
        self.consume(&mut wb, false);
        Ok(())
    }

    fn gc(&self, raft_group_id: u64, from: u64, to: u64) -> Result<usize> {
        let mut raft_wb = self.log_batch(0);
        for idx in from..to {
            raft_wb.del_page(raft_group_id, idx);
        }
        self.consume(&mut raft_wb, false);
        Ok((from - to) as usize)
    }

    fn batch_gc(&self, groups: Vec<RaftLogGCTask>) -> Result<usize> {
        let mut total = 0;
        let mut raft_wb = self.log_batch(0);
        for task in groups {
            for idx in task.from..task.to {
                raft_wb.del_page(task.raft_group_id, idx);
            }
            total += (task.to - task.from) as usize;
        }
        self.consume(&mut raft_wb, false);
        Ok(total)
    }

    fn purge_expired_files(&self) -> Result<Vec<u64>> {
        Ok(vec![])
    }

    fn has_builtin_entry_cache(&self) -> bool {
        false
    }

    fn flush_metrics(&self, instance: &str) {
    }

    fn reset_statistics(&self) {
    }

    fn dump_stats(&self) -> Result<String> {
        Ok(String::from(""))
    }

    fn get_engine_size(&self) -> Result<u64> {
        Ok(0)
    }
}
