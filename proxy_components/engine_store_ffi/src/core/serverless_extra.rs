// Copyright 2024 TiKV Project Authors. Licensed under Apache-2.0.

use std::convert::TryInto;

use crate::{
    core::common::*,
    ffi::interfaces_ffi::{FastAddPeerRes, FastAddPeerStatus},
};

#[derive(Default)]
pub struct ServerlessExtra {
    shard_ver: u64,
    inner_key: Vec<u8>,
    enc_key: Vec<u8>,
    txn_file_ref: Vec<u8>,
}

impl ServerlessExtra {
    pub fn new(res: &FastAddPeerRes) -> Self {
        Default::default()
    }
}