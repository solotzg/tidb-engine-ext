// Copyright 2024 TiKV Project Authors. Licensed under Apache-2.0.

use std::path::Path;

use tempfile::NamedTempFile;

use crate::utils::v1::*;

#[test]
fn test_adhoc_dump_prof() {
    use proxy_server::status_server::vendored_utils::{
        activate_prof, deactivate_prof, has_activate_prof,
    };

    let prev_has_activate_prof = has_activate_prof();
    if !prev_has_activate_prof {
        let _ = activate_prof();
    }

    let x = vec![1; 1000];
    let y = vec![1; 1000];

    let f = NamedTempFile::new().unwrap();
    let path = f.path().to_str().unwrap();
    proxy_server::status_server::vendored_utils::adhoc_dump(path).unwrap();
    let target_path = Path::new(path);
    assert_eq!(target_path.exists(), true);
    assert!(std::fs::metadata(path).unwrap().len() > 1000);

    if !prev_has_activate_prof {
        let _ = deactivate_prof();
    }
}
