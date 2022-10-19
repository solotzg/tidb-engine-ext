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

    fn put_page(&mut self, page_id: &[u8], value: &[u8]) -> Result<()> {
        let helper = gen_engine_store_server_helper(self.engine_store_server_helper);
        helper.write_batch_put_page(self.raw_write_batch.ptr, page_id.into(), value.into());
        Ok(())
    }

    fn del_page(&mut self, page_id: &[u8]) -> Result<()> {
        let helper = gen_engine_store_server_helper(self.engine_store_server_helper);
        helper.write_batch_del_page(self.raw_write_batch.ptr, page_id.into());
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
            let key = keys::raft_log_key(raft_group_id, entry.get_index());
            self.put_page(&key, &ser_buf)?;
        }
        Ok(())
    }

    fn put_msg<M: protobuf::Message>(&mut self, page_id: &[u8], m: &M) -> Result<()> {
        self.put_page(page_id, &m.write_to_bytes()?)
    }

    fn data_size(&self) -> usize {
        let helper = gen_engine_store_server_helper(self.engine_store_server_helper);
        return helper.write_batch_size(self.raw_write_batch.ptr) as usize;
    }

    fn clear(&self) {
        let helper = gen_engine_store_server_helper(self.engine_store_server_helper);
        helper.write_batch_clear(self.raw_write_batch.ptr);
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
            let key = keys::raft_log_key(raft_group_id, index);
            self.del_page(&key).unwrap();
        }
    }

    fn put_raft_state(&mut self, raft_group_id: u64, state: &RaftLocalState) -> Result<()> {
        self.put_msg(&keys::raft_state_key(raft_group_id), state)
    }

    fn persist_size(&self) -> usize {
        self.data_size()
    }

    fn is_empty(&self) -> bool {
        let helper = gen_engine_store_server_helper(self.engine_store_server_helper);
        helper.write_batch_is_empty(self.raw_write_batch.ptr) == 0
    }

    fn merge(&mut self, src: Self) -> Result<()> {
        let helper = gen_engine_store_server_helper(self.engine_store_server_helper);
        helper.write_batch_merge(self.raw_write_batch.ptr, src.raw_write_batch.ptr);
        Ok(())
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
        page_id: &[u8],
    ) -> Result<Option<M>> {
        let helper = gen_engine_store_server_helper(self.engine_store_server_helper);
        let value = helper.read_page(page_id.into());
        if value.view.len == 0 {
            return Ok(None);
        }

        let mut m = M::default();
        m.merge_from_bytes(unsafe { slice::from_raw_parts(value.view.data as *const u8, value.view.len as usize) })?;
        Ok(Some(m))
    }

    fn get_value(
        &self,
        page_id: &[u8],
    ) -> Option<Vec<u8>> {
        let helper = gen_engine_store_server_helper(self.engine_store_server_helper);
        let value = helper.read_page(page_id.into());
        return if value.view.len == 0 {
            None
        } else {
            Some(value.view.to_slice().to_vec())
        }
    }

    /// scan the key between start_key(inclusive) and end_key(exclusive),
    /// the upper bound is omitted if end_key is empty
    fn scan<F>(&self, start_key: &[u8], end_key: &[u8], mut f: F) -> Result<()>
        where
            F: FnMut(&[u8], &[u8]) -> Result<bool>,
    {
        let helper = gen_engine_store_server_helper(self.engine_store_server_helper);
        let values = helper.scan_page(start_key.into(), end_key.into());
        for i in 0..values.len {
            let value = unsafe {
                &*values.inner.offset(i as isize)
            };
            if value.view.len != 0 {
                if !f(&[], &value.view.to_slice().to_vec())? {
                    break;
                }
            }
        }
        Ok(())
    }
}

impl RaftEngineReadOnly for PSEngine {
    fn get_raft_state(&self, raft_group_id: u64) -> Result<Option<RaftLocalState>> {
        let key = keys::raft_state_key(raft_group_id);
        self.get_msg_cf(&key)
    }

    fn get_entry(&self, raft_group_id: u64, index: u64) -> Result<Option<Entry>> {
        let key = keys::raft_log_key(raft_group_id, index);
        self.get_msg_cf(&key)
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

        let start_key = keys::raft_log_key(region_id, low);
        let end_key = keys::raft_log_key(region_id, high);

        let mut count = 1;

        self.scan(
            &start_key,
            &end_key,
            |_, page| {
                let mut entry = Entry::default();
                entry.merge_from_bytes(page)?;
                buf.push(entry);
                total_size += page.len();
                count += 1;
                Ok(total_size < max_size)
            },
        )?;

        return Ok(count);
    }

    fn get_all_entries_to(&self, region_id: u64, buf: &mut Vec<Entry>) -> Result<()> {
        let start_key = keys::raft_log_key(region_id, 0);
        let end_key = keys::raft_log_key(region_id, u64::MAX);
        self.scan(
            &start_key,
            &end_key,
            |_, page| {
                let mut entry = Entry::default();
                entry.merge_from_bytes(page)?;
                buf.push(entry);
                Ok(true)
            },
        )?;
        Ok(())
    }
}

impl RaftEngineDebug for PSEngine {
    fn scan_entries<F>(&self, raft_group_id: u64, mut f: F) -> Result<()>
        where
            F: FnMut(&Entry) -> Result<bool>,
    {
        let start_key = keys::raft_log_key(raft_group_id, 0);
        let end_key = keys::raft_log_key(raft_group_id, u64::MAX);
        self.scan(
            &start_key,
            &end_key,
            |_, value| {
                let mut entry = Entry::default();
                entry.merge_from_bytes(value)?;
                f(&entry)
            },
        );
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
        self.consume(batch, sync_log)
    }

    fn clean(
        &self,
        raft_group_id: u64,
        mut first_index: u64,
        state: &RaftLocalState,
        batch: &mut Self::LogBatch,
    ) -> Result<()> {
        batch.del_page(&keys::raft_state_key(raft_group_id))?;
        // TODO: find the first raft log index of this raft group
        if first_index <= state.last_index {
            for index in first_index..=state.last_index {
                batch.del_page( &keys::raft_log_key(raft_group_id, index));
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
        wb.put_msg(&keys::raft_state_key(raft_group_id), state);
        self.consume(&mut wb, false);
        Ok(())
    }

    fn gc(&self, raft_group_id: u64, from: u64, to: u64) -> Result<usize> {
        let mut raft_wb = self.log_batch(0);
        for idx in from..to {
            raft_wb.del_page(&keys::raft_log_key(raft_group_id, idx));
        }
        self.consume(&mut raft_wb, false);
        Ok((from - to) as usize)
    }

    fn batch_gc(&self, groups: Vec<RaftLogGCTask>) -> Result<usize> {
        let mut total = 0;
        let mut raft_wb = self.log_batch(0);
        for task in groups {
            for idx in task.from..task.to {
                raft_wb.del_page(&keys::raft_log_key(task.raft_group_id, idx));
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
