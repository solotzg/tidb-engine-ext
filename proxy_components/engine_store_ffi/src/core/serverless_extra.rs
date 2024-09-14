// Copyright 2024 TiKV Project Authors. Licensed under Apache-2.0.

use std::convert::TryInto;

use crate::{
    core::common::*,
    ffi::interfaces_ffi::{FastAddPeerRes, FastAddPeerStatus},
};

pub struct ServerlessExtra {
    shard_ver: u64,
    inner_key: Vec<u8>,
    enc_key: Vec<u8>,
    txn_file_ref: Vec<u8>,
}

impl ServerlessExtra {
    // TODO TXN_FILE_REF
    pub fn new(res: &FastAddPeerRes) -> Self {
        Self {
            shard_ver: res.shard_ver,
            inner_key: res.inner_key.view.to_slice().to_vec(),
            enc_key: res.enc_key.view.to_slice().to_vec(),
            txn_file_ref: res.txn_file_ref.view.to_slice().to_vec(),
        }
    }

    pub fn mutate_snap(&self, shard_id: u64, changeset: &mut kvenginepb::ChangeSet) {
        let snap = changeset.mut_snapshot();
        {
            let props = kvengine::Properties::default();
            props.set(kvengine::ENCRYPTION_KEY, self.get_enc_key());
            props.set(kvengine::TXN_FILE_REF, self.get_txn_file_ref());
            snap.set_properties(props.to_pb(shard_id));
            snap.set_inner_key_off(self.get_inner_key_off());
        }
        changeset.set_shard_ver(self.get_shard_ver());
    }

    fn get_shard_ver(&self) -> u64 {
        self.shard_ver
    }

    fn get_inner_key_off(&self) -> u32 {
        if self.inner_key.len() < 4 {
            info!("fast path: get error inner key {:?}", self.inner_key);
            return 0;
        }
        u32::from_be_bytes(self.inner_key[0..4].try_into().unwrap())
    }

    fn get_enc_key(&self) -> &[u8] {
        &self.enc_key
    }

    fn get_txn_file_ref(&self) -> &[u8] {
        // load_peer_txn_file_locks
        &self.txn_file_ref
    }
}