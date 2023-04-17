// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.
use std::sync::Arc;

use encryption::DataKeyManager;
use engine_rocks::{RocksCfOptions, RocksDbOptions};
use engine_traits::{Iterable, Iterator, CF_DEFAULT, CF_LOCK, CF_WRITE};

use crate::{
    cf_to_name,
    interfaces_ffi::{
        BaseBuffView, ColumnFamilyType, EngineIteratorSeekType, SSTFormatKind, SSTReaderPtr,
    },
};

pub struct TabletReader {
    kv_engine: engine_rocks::RocksEngine,
    iterator: engine_rocks::RocksEngineIterator,
}

impl TabletReader {
    pub fn ffi_get_cf_file_reader(
        path: &str,
        cf: ColumnFamilyType,
        _key_manager: Option<Arc<DataKeyManager>>,
    ) -> SSTReaderPtr {
        let mut db_opts = RocksDbOptions::default();
        let defaultcf = RocksCfOptions::default();
        let lockcf = RocksCfOptions::default();
        let writecf = RocksCfOptions::default();
        let cf_opts = vec![
            (CF_DEFAULT, defaultcf),
            (CF_LOCK, lockcf),
            (CF_WRITE, writecf),
        ];
        let kv_engine = engine_rocks::util::new_engine_opt(path, db_opts, cf_opts);
        if let Err(e) = &kv_engine {
            tikv_util::error!("failed to read tablet snapshot"; "path" => path, "err" => ?e);
        }
        let kv_engine = kv_engine.unwrap();
        let cf_name = cf_to_name(cf);
        let iterator = kv_engine.iterator(cf_name).unwrap();
        let tr = Box::new(TabletReader {
            kv_engine,
            iterator,
        });
        SSTReaderPtr {
            inner: Box::into_raw(tr) as *mut _,
            kind: SSTFormatKind::KIND_TABLET,
        }
    }

    pub fn ffi_remained(&self) -> u8 {
        todo!()
    }

    pub fn ffi_key(&self) -> BaseBuffView {
        todo!()
    }

    pub fn ffi_val(&self) -> BaseBuffView {
        todo!()
    }

    pub fn ffi_next(&mut self) {
        todo!()
        // let s = self.iterator.seek().unwrap();
        // for &(k, v) in expected {
        //     assert_eq!(k, iter.key());
        //     assert_eq!(v, iter.value());
        //     iter.next().unwrap();
        // }
        // assert!(!iter.valid().unwrap());
    }

    pub fn ffi_seek(&self, _: ColumnFamilyType, _: EngineIteratorSeekType, _: BaseBuffView) {
        todo!()
    }
}
