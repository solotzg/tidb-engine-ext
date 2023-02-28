// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

#![allow(unused_variables)]

use engine_rocks::RocksEngineIterator;
use engine_traits::{IterOptions, Iterable, Peekable, ReadOptions, Result};
use rocksdb::Writable;

use crate::{
    mixed_engine::{elementary::ElementaryEngine, MixedDbVector},
    r2e,
    util::get_cf_handle,
    RocksEngine,
};

#[derive(Clone, Debug)]
pub struct RocksElementEngine {
    pub rocks: engine_rocks::RocksEngine,
}

unsafe impl Send for RocksElementEngine {}
unsafe impl Sync for RocksElementEngine {}

impl ElementaryEngine for RocksElementEngine {
    fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        self.rocks.get_sync_db().put(key, value).map_err(r2e)
    }

    fn put_cf(&self, cf: &str, key: &[u8], value: &[u8]) -> Result<()> {
        let db = self.rocks.get_sync_db();
        let handle = get_cf_handle(&db, cf)?;
        self.rocks
            .get_sync_db()
            .put_cf(handle, key, value)
            .map_err(r2e)
    }

    fn delete(&self, key: &[u8]) -> Result<()> {
        self.rocks.get_sync_db().delete(key).map_err(r2e)
    }

    fn delete_cf(&self, cf: &str, key: &[u8]) -> Result<()> {
        let db = self.rocks.get_sync_db();
        let handle = get_cf_handle(&db, cf)?;
        self.rocks.get_sync_db().delete_cf(handle, key).map_err(r2e)
    }

    fn get_value_opt(&self, opts: &ReadOptions, key: &[u8]) -> Result<Option<MixedDbVector>> {
        let o = self.rocks.get_value_opt(opts, key)?;
        if let Some(r) = o {
            return Ok(Some(MixedDbVector::from_raw_rocks(r)));
        }
        Ok(None)
    }

    fn get_value_cf_opt(
        &self,
        opts: &ReadOptions,
        cf: &str,
        key: &[u8],
    ) -> Result<Option<MixedDbVector>> {
        let o = self.rocks.get_value_cf_opt(opts, cf, key)?;
        if let Some(r) = o {
            return Ok(Some(MixedDbVector::from_raw_rocks(r)));
        }
        Ok(None)
    }
}

impl Iterable for RocksEngine {
    type Iterator = RocksEngineIterator;

    fn iterator_opt(&self, cf: &str, opts: IterOptions) -> Result<Self::Iterator> {
        self.rocks.iterator_opt(cf, opts)
    }
}
