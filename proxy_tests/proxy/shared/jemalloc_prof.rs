// Copyright 2024 TiKV Project Authors. Licensed under Apache-2.0.

use tempfile::{NamedTempFile};
use std::path::Path;
use crate::utils::v1::*;

#[test]
fn test_adhoc_dump_prof() {
    let f = NamedTempFile::new().unwrap();
    let path = f.path().to_str().unwrap();
    println!("!!!!! XXXX {}", path);
    proxy_server::status_server::vendored_utils::adhoc_dump(path).unwrap();
    let target_path = Path::new(path);
    assert_eq!(target_path.exists(), true);
    std::thread::sleep(std::time::Duration::from_millis(100000));
    panic!("AAAAA");
}