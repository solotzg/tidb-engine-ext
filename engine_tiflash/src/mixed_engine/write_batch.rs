// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

#![allow(unused_variables)]
use std::sync::Arc;

use engine_traits::{self, Mutable, Result, WriteBatchExt, WriteOptions};
use proxy_ffi::interfaces_ffi::RawCppPtr;
use rocksdb::{WriteBatch as RawWriteBatch, DB};
use tikv_util::Either;

use crate::{engine::RocksEngine, ps_engine::add_prefix, r2e, PageStorageExt};

pub const WRITE_BATCH_MAX_BATCH: usize = 16;
pub const WRITE_BATCH_LIMIT: usize = 16;

pub struct MixedWriteBatch {
    pub inner:
        Either<crate::rocks_engine::RocksWriteBatchVec, crate::ps_engine::PSRocksWriteBatchVec>,
}

impl WriteBatchExt for RocksEngine {
    type WriteBatch = MixedWriteBatch;

    const WRITE_BATCH_MAX_KEYS: usize = 256;

    fn write_batch(&self) -> MixedWriteBatch {
        self.element_engine.as_ref().unwrap().write_batch()
    }

    fn write_batch_with_cap(&self, cap: usize) -> MixedWriteBatch {
        self.element_engine
            .as_ref()
            .unwrap()
            .write_batch_with_cap(cap)
    }
}

impl engine_traits::WriteBatch for MixedWriteBatch {
    fn write_opt(&mut self, opts: &WriteOptions) -> Result<u64> {
        // write into ps
        match self.inner.as_mut() {
            Either::Left(x) => x.write_opt(opts),
            Either::Right(x) => x.write_opt(opts),
        }
    }

    fn data_size(&self) -> usize {
        match self.inner.as_ref() {
            Either::Left(x) => x.data_size(),
            Either::Right(x) => x.data_size(),
        }
    }

    fn count(&self) -> usize {
        match self.inner.as_ref() {
            Either::Left(x) => x.count(),
            Either::Right(x) => x.count(),
        }
    }

    fn is_empty(&self) -> bool {
        match self.inner.as_ref() {
            Either::Left(x) => x.is_empty(),
            Either::Right(x) => x.is_empty(),
        }
    }

    fn should_write_to_engine(&self) -> bool {
        // Disable TiKV's logic, and using Proxy's instead.
        false
    }

    fn clear(&mut self) {
        match self.inner.as_mut() {
            Either::Left(x) => x.clear(),
            Either::Right(x) => x.clear(),
        }
    }

    fn set_save_point(&mut self) {
        self.wbs[self.index].set_save_point();
        self.save_points.push(self.index);
    }

    fn pop_save_point(&mut self) -> Result<()> {
        if let Some(x) = self.save_points.pop() {
            return self.wbs[x].pop_save_point().map_err(r2e);
        }
        Err(r2e("no save point"))
    }

    fn rollback_to_save_point(&mut self) -> Result<()> {
        if let Some(x) = self.save_points.pop() {
            for i in x + 1..=self.index {
                self.wbs[i].clear();
            }
            self.index = x;
            return self.wbs[x].rollback_to_save_point().map_err(r2e);
        }
        Err(r2e("no save point"))
    }

    fn merge(&mut self, other: Self) -> Result<()> {
        match self.inner.as_mut() {
            Either::Left(x) => x.merge(other.left().unwrap()),
            Either::Right(x) => x.merge(other.right().unwrap()),
        }
    }
}

impl Mutable for MixedWriteBatch {
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        if !self.do_write(engine_traits::CF_DEFAULT, key) {
            return Ok(());
        }
        match self.inner.as_mut() {
            Either::Left(x) => x.put(key, value),
            Either::Right(x) => x.put(key, value),
        }
    }

    fn put_cf(&mut self, cf: &str, key: &[u8], value: &[u8]) -> Result<()> {
        if !self.do_write(cf, key) {
            return Ok(());
        }
        match self.inner.as_mut() {
            Either::Left(x) => x.put_cf(cf, key, value),
            Either::Right(x) => x.put_cf(cf, key, value),
        }
    }

    fn delete(&mut self, key: &[u8]) -> Result<()> {
        if !self.do_write(engine_traits::CF_DEFAULT, key) {
            return Ok(());
        }
        match self.inner.as_mut() {
            Either::Left(x) => x.delete(key),
            Either::Right(x) => x.delete(key),
        }
    }

    fn delete_cf(&mut self, cf: &str, key: &[u8]) -> Result<()> {
        if !self.do_write(cf, key) {
            return Ok(());
        }
        match self.inner.as_mut() {
            Either::Left(x) => x.delete_cf(cf, key),
            Either::Right(x) => x.delete_cf(cf, key),
        }
    }

    fn delete_range(&mut self, begin_key: &[u8], end_key: &[u8]) -> Result<()> {
        Ok(())
    }

    fn delete_range_cf(&mut self, cf: &str, begin_key: &[u8], end_key: &[u8]) -> Result<()> {
        Ok(())
    }
}
