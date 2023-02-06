// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.
use crate::{
    core::{common::*, PrehandleTask, ProxyForwarder, PtrWrapper},
    fatal,
};

impl<T: Transport + 'static, ER: RaftEngine> ProxyForwarder<T, ER> {
    fn handle_ingest_sst_for_engine_store(
        &self,
        ob_region: &Region,
        ssts: &Vec<engine_traits::SstMetaInfo>,
        index: u64,
        term: u64,
    ) -> EngineStoreApplyRes {
        let mut ssts_wrap = vec![];
        let mut sst_views = vec![];

        info!("begin handle ingest sst";
            "region" => ?ob_region,
            "index" => index,
            "term" => term,
        );

        for sst in ssts {
            let sst = &sst.meta;
            if sst.get_cf_name() == engine_traits::CF_LOCK {
                panic!("should not ingest sst of lock cf");
            }

            // We still need this to filter error ssts.
            if let Err(e) = check_sst_for_ingestion(sst, ob_region) {
                error!(?e;
                 "proxy ingest fail";
                 "sst" => ?sst,
                 "region" => ?ob_region,
                );
                break;
            }

            ssts_wrap.push((
                self.sst_importer.get_path(sst),
                name_to_cf(sst.get_cf_name()),
            ));
        }

        for (path, cf) in &ssts_wrap {
            sst_views.push((path.to_str().unwrap().as_bytes(), *cf));
        }

        let res = self.engine_store_server_helper.handle_ingest_sst(
            sst_views,
            RaftCmdHeader::new(ob_region.get_id(), index, term),
        );
        res
    }

    fn handle_error_apply(
        &self,
        ob_region: &Region,
        cmd: &Cmd,
        region_state: &RegionState,
    ) -> bool {
        // We still need to pass a dummy cmd, to forward updates.
        let cmd_dummy = WriteCmds::new();
        let flash_res = self.engine_store_server_helper.handle_write_raft_cmd(
            &cmd_dummy,
            RaftCmdHeader::new(ob_region.get_id(), cmd.index, cmd.term),
        );
        match flash_res {
            EngineStoreApplyRes::None => false,
            EngineStoreApplyRes::Persist => !region_state.pending_remove,
            EngineStoreApplyRes::NotFound => false,
        }
    }

    pub fn pre_exec_admin(
        &self,
        ob_region: &Region,
        req: &AdminRequest,
        index: u64,
        term: u64,
    ) -> bool {
        match req.get_cmd_type() {
            AdminCmdType::CompactLog => {
                if !self.engine_store_server_helper.try_flush_data(
                    ob_region.get_id(),
                    false,
                    false,
                    index,
                    term,
                ) {
                    info!("can't flush data, filter CompactLog";
                        "region_id" => ?ob_region.get_id(),
                        "region_epoch" => ?ob_region.get_region_epoch(),
                        "index" => index,
                        "term" => term,
                        "compact_index" => req.get_compact_log().get_compact_index(),
                        "compact_term" => req.get_compact_log().get_compact_term(),
                    );
                    return true;
                }
                // Otherwise, we can exec CompactLog, without later rolling
                // back.
            }
            AdminCmdType::ComputeHash | AdminCmdType::VerifyHash => {
                // We can't support.
                return true;
            }
            AdminCmdType::TransferLeader => {
                error!("transfer leader won't exec";
                        "region" => ?ob_region,
                        "req" => ?req,
                );
                return true;
            }
            _ => (),
        };
        false
    }

    pub fn post_exec_admin(
        &self,
        ob_region: &Region,
        cmd: &Cmd,
        apply_state: &RaftApplyState,
        region_state: &RegionState,
        _: &mut ApplyCtxInfo<'_>,
    ) -> bool {
        fail::fail_point!("on_post_exec_admin", |e| {
            e.unwrap().parse::<bool>().unwrap()
        });
        let region_id = ob_region.get_id();
        let request = cmd.request.get_admin_request();
        let response = &cmd.response;
        let admin_reponse = response.get_admin_response();
        let cmd_type = request.get_cmd_type();

        if response.get_header().has_error() {
            info!(
                "error occurs when apply_admin_cmd, {:?}",
                response.get_header().get_error()
            );
            return self.handle_error_apply(ob_region, cmd, region_state);
        }

        match cmd_type {
            AdminCmdType::CompactLog | AdminCmdType::ComputeHash | AdminCmdType::VerifyHash => {
                info!(
                    "observe useless admin command";
                    "region_id" => region_id,
                    "peer_id" => region_state.peer_id,
                    "term" => cmd.term,
                    "index" => cmd.index,
                    "type" => ?cmd_type,
                );
            }
            _ => {
                info!(
                    "observe admin command";
                    "region_id" => region_id,
                    "peer_id" => region_state.peer_id,
                    "term" => cmd.term,
                    "index" => cmd.index,
                    "command" => ?request
                );
            }
        }

        // We wrap `modified_region` into `mut_split()`
        let mut new_response = None;
        match cmd_type {
            AdminCmdType::CommitMerge
            | AdminCmdType::PrepareMerge
            | AdminCmdType::RollbackMerge => {
                let mut r = AdminResponse::default();
                match region_state.modified_region.as_ref() {
                    Some(region) => r.mut_split().set_left(region.clone()),
                    None => {
                        error!("empty modified region";
                            "region_id" => region_id,
                            "peer_id" => region_state.peer_id,
                            "term" => cmd.term,
                            "index" => cmd.index,
                            "command" => ?request
                        );
                        panic!("empty modified region");
                    }
                }
                new_response = Some(r);
            }
            _ => (),
        }

        let flash_res = {
            match new_response {
                Some(r) => self.engine_store_server_helper.handle_admin_raft_cmd(
                    request,
                    &r,
                    RaftCmdHeader::new(region_id, cmd.index, cmd.term),
                ),
                None => self.engine_store_server_helper.handle_admin_raft_cmd(
                    request,
                    admin_reponse,
                    RaftCmdHeader::new(region_id, cmd.index, cmd.term),
                ),
            }
        };
        let persist = match flash_res {
            EngineStoreApplyRes::None => {
                if cmd_type == AdminCmdType::CompactLog {
                    // This could only happen in mock-engine-store when we perform some related
                    // tests. Formal code should never return None for
                    // CompactLog now. If CompactLog can't be done, the
                    // engine-store should return `false` in previous `try_flush_data`.
                    error!("applying CompactLog should not return None"; "region_id" => region_id,
                            "peer_id" => region_state.peer_id, "apply_state" => ?apply_state, "cmd" => ?cmd);
                }
                false
            }
            EngineStoreApplyRes::Persist => !region_state.pending_remove,
            EngineStoreApplyRes::NotFound => {
                error!(
                    "region not found in engine-store, maybe have exec `RemoveNode` first";
                    "region_id" => region_id,
                    "peer_id" => region_state.peer_id,
                    "term" => cmd.term,
                    "index" => cmd.index,
                );
                !region_state.pending_remove
            }
        };
        if persist {
            info!("should persist admin"; "region_id" => region_id, "peer_id" => region_state.peer_id, "state" => ?apply_state);
        }
        persist
    }

    pub fn on_empty_cmd(&self, ob_region: &Region, index: u64, term: u64) {
        let region_id = ob_region.get_id();
        fail::fail_point!("on_empty_cmd_normal", |_| {});
        debug!("encounter empty cmd, maybe due to leadership change";
            "region" => ?ob_region,
            "index" => index,
            "term" => term,
        );
        // We still need to pass a dummy cmd, to forward updates.
        let cmd_dummy = WriteCmds::new();
        self.engine_store_server_helper
            .handle_write_raft_cmd(&cmd_dummy, RaftCmdHeader::new(region_id, index, term));
    }

    pub fn post_exec_query(
        &self,
        ob_region: &Region,
        cmd: &Cmd,
        apply_state: &RaftApplyState,
        region_state: &RegionState,
        apply_ctx_info: &mut ApplyCtxInfo<'_>,
    ) -> bool {
        fail::fail_point!("on_post_exec_normal", |e| {
            e.unwrap().parse::<bool>().unwrap()
        });
        let region_id = ob_region.get_id();
        const NONE_STR: &str = "";
        let requests = cmd.request.get_requests();
        let response = &cmd.response;
        if response.get_header().has_error() {
            let proto_err = response.get_header().get_error();
            if proto_err.has_flashback_in_progress() {
                debug!(
                    "error occurs when apply_write_cmd, {:?}",
                    response.get_header().get_error()
                );
            } else {
                info!(
                    "error occurs when apply_write_cmd, {:?}",
                    response.get_header().get_error()
                );
            }
            return self.handle_error_apply(ob_region, cmd, region_state);
        }

        let mut ssts = vec![];
        let mut cmds = WriteCmds::with_capacity(requests.len());
        for req in requests {
            let cmd_type = req.get_cmd_type();
            match cmd_type {
                CmdType::Put => {
                    let put = req.get_put();
                    let cf = name_to_cf(put.get_cf());
                    let (key, value) = (put.get_key(), put.get_value());
                    cmds.push(key, value, WriteCmdType::Put, cf);
                }
                CmdType::Delete => {
                    let del = req.get_delete();
                    let cf = name_to_cf(del.get_cf());
                    let key = del.get_key();
                    cmds.push(key, NONE_STR.as_ref(), WriteCmdType::Del, cf);
                }
                CmdType::IngestSst => {
                    ssts.push(engine_traits::SstMetaInfo {
                        total_bytes: 0,
                        total_kvs: 0,
                        meta: req.get_ingest_sst().get_sst().clone(),
                    });
                }
                CmdType::Snap | CmdType::Get | CmdType::DeleteRange => {
                    // engine-store will drop table, no need DeleteRange
                    // We will filter delete range in engine_tiflash
                    continue;
                }
                CmdType::Prewrite | CmdType::Invalid | CmdType::ReadIndex => {
                    panic!("invalid cmd type, message maybe corrupted");
                }
            }
        }

        let persist = if !ssts.is_empty() {
            assert_eq!(cmds.len(), 0);
            match self.handle_ingest_sst_for_engine_store(ob_region, &ssts, cmd.index, cmd.term) {
                EngineStoreApplyRes::None => {
                    // Before, BR/Lightning may let ingest sst cmd contain only one cf,
                    // which may cause that TiFlash can not flush all region cache into column.
                    // so we have a optimization proxy@cee1f003.
                    // The optimization is to introduce a `pending_delete_ssts`,
                    // which holds ssts from being cleaned(by adding into `delete_ssts`),
                    // when engine-store returns None.
                    // Though this is fixed by br#1150 & tikv#10202, we still have to handle None,
                    // since TiKV's compaction filter can also cause mismatch between default and
                    // write. According to tiflash#1811.
                    // Since returning None will cause no persistence of advanced apply index,
                    // So in a recovery, we can replay ingestion in `pending_delete_ssts`,
                    // thus leaving no un-tracked sst files.

                    // We must hereby move all ssts to `pending_delete_ssts` for protection.
                    match apply_ctx_info.pending_handle_ssts {
                        None => (), // No ssts to handle, unlikely.
                        Some(v) => {
                            self.pending_delete_ssts
                                .write()
                                .expect("lock error")
                                .append(v);
                        }
                    };
                    info!(
                        "skip persist for ingest sst";
                        "region_id" => region_id,
                        "peer_id" => region_state.peer_id,
                        "term" => cmd.term,
                        "index" => cmd.index,
                        "ssts_to_clean" => ?ssts,
                    );
                    false
                }
                EngineStoreApplyRes::NotFound | EngineStoreApplyRes::Persist => {
                    info!(
                        "ingest sst success";
                        "region_id" => region_id,
                        "peer_id" => region_state.peer_id,
                        "term" => cmd.term,
                        "index" => cmd.index,
                        "ssts_to_clean" => ?ssts,
                    );
                    match apply_ctx_info.pending_handle_ssts {
                        None => (),
                        Some(v) => {
                            let mut sst_in_region: Vec<SstMetaInfo> = self
                                .pending_delete_ssts
                                .write()
                                .expect("lock error")
                                .drain_filter(|e| e.meta.get_region_id() == region_id)
                                .collect();
                            apply_ctx_info.delete_ssts.append(&mut sst_in_region);
                            apply_ctx_info.delete_ssts.append(v);
                        }
                    }
                    !region_state.pending_remove
                }
            }
        } else {
            let flash_res = {
                self.engine_store_server_helper.handle_write_raft_cmd(
                    &cmds,
                    RaftCmdHeader::new(region_id, cmd.index, cmd.term),
                )
            };
            match flash_res {
                EngineStoreApplyRes::None => false,
                EngineStoreApplyRes::Persist => !region_state.pending_remove,
                EngineStoreApplyRes::NotFound => false,
            }
        };
        fail::fail_point!("on_post_exec_normal_end", |e| {
            e.unwrap().parse::<bool>().unwrap()
        });
        if persist {
            info!("should persist query"; "region_id" => region_id, "peer_id" => region_state.peer_id, "state" => ?apply_state);
        }
        persist
    }

    pub fn on_update_safe_ts(&self, region_id: u64, self_safe_ts: u64, leader_safe_ts: u64) {
        self.engine_store_server_helper.handle_safe_ts_update(
            region_id,
            self_safe_ts,
            leader_safe_ts,
        )
    }

    pub fn on_region_changed(&self, ob_region: &Region, e: RegionChangeEvent, _: StateRole) {
        let region_id = ob_region.get_id();
        if e == RegionChangeEvent::Destroy {
            info!(
                "observe destroy";
                "region_id" => region_id,
                "store_id" => self.store_id,
            );
            self.engine_store_server_helper.handle_destroy(region_id);
            if self.packed_envs.engine_store_cfg.enable_fast_add_peer {
                self.get_cached_manager()
                    .remove_cached_region_info(region_id);
            }
        }
    }

    #[allow(clippy::match_like_matches_macro)]
    pub fn pre_persist(
        &self,
        ob_region: &Region,
        is_finished: bool,
        cmd: Option<&RaftCmdRequest>,
    ) -> bool {
        let region_id = ob_region.get_id();
        let should_persist = if is_finished {
            true
        } else {
            let cmd = cmd.unwrap();
            if cmd.has_admin_request() {
                match cmd.get_admin_request().get_cmd_type() {
                    // Merge needs to get the latest apply index.
                    AdminCmdType::CommitMerge | AdminCmdType::RollbackMerge => true,
                    _ => false,
                }
            } else {
                false
            }
        };
        if should_persist {
            debug!(
            "observe pre_persist, persist";
            "region_id" => region_id,
            "store_id" => self.store_id,
            );
        } else {
            debug!(
            "observe pre_persist";
            "region_id" => region_id,
            "store_id" => self.store_id,
            "is_finished" => is_finished,
            );
        };
        should_persist
    }

    pub fn pre_write_apply_state(&self, _ob_region: &Region) -> bool {
        fail::fail_point!("on_pre_persist_with_finish", |_| {
            // Some test need persist apply state for Leader logic,
            // including fast add peer.
            true
        });
        false
    }

    pub fn on_raft_message(&self, msg: &RaftMessage) -> bool {
        !self.maybe_fast_path(&msg)
    }

    pub fn on_role_change(&self, ob_region: &Region, r: &RoleChange) {
        let region_id = ob_region.get_id();
        let is_replicated = !r.initialized;
        let f = |info: MapEntry<u64, Arc<CachedRegionInfo>>| match info {
            MapEntry::Occupied(mut o) => {
                // Note the region info may be registered by maybe_fast_path
                info!("fast path: ongoing {}:{} {}, peer created",
                    self.store_id, region_id, 0;
                    "region_id" => region_id,
                    "is_replicated" => is_replicated,
                );
                if is_replicated {
                    o.get_mut()
                        .replicated_or_created
                        .store(true, Ordering::SeqCst);
                }
            }
            MapEntry::Vacant(v) => {
                // TODO support peer_id
                info!("fast path: ongoing {}:{} {}, peer created",
                    self.store_id, region_id, 0;
                    "region_id" => region_id,
                    "is_replicated" => is_replicated,
                );
                if is_replicated {
                    let c = CachedRegionInfo::default();
                    c.replicated_or_created.store(true, Ordering::SeqCst);
                    v.insert(Arc::new(c));
                }
            }
        };
        // TODO remove unwrap
        self.get_cached_manager()
            .access_cached_region_info_mut(region_id, f)
            .unwrap();
    }

    #[allow(clippy::single_match)]
    pub fn pre_apply_snapshot(
        &self,
        ob_region: &Region,
        peer_id: u64,
        snap_key: &store::SnapKey,
        snap: Option<&store::Snapshot>,
    ) {
        let region_id = ob_region.get_id();
        info!("pre apply snapshot";
            "peer_id" => peer_id,
            "region_id" => region_id,
            "snap_key" => ?snap_key,
            "has_snap" => snap.is_some(),
            "pending" => self.engine.pending_applies_count.load(Ordering::SeqCst),
        );
        fail::fail_point!("on_ob_pre_handle_snapshot", |_| {});

        let snap = match snap {
            None => return,
            Some(s) => s,
        };

        fail::fail_point!("on_ob_pre_handle_snapshot_delete", |_| {
            let ssts = retrieve_sst_files(snap);
            for (pathbuf, _) in ssts.iter() {
                debug!("delete snapshot file"; "path" => ?pathbuf);
                std::fs::remove_file(pathbuf.as_path()).unwrap();
            }
            return;
        });

        let mut should_skip = false;
        #[allow(clippy::collapsible_if)]
        if self.packed_envs.engine_store_cfg.enable_fast_add_peer {
            if self.get_cached_manager().access_cached_region_info_mut(
                region_id,
                |info: MapEntry<u64, Arc<CachedRegionInfo>>| match info {
                    MapEntry::Occupied(o) => {
                        let is_first_snapsot = !o.get().inited_or_fallback.load(Ordering::SeqCst);
                        if is_first_snapsot {
                            info!("fast path: prehandle first snapshot {}:{} {}", self.store_id, region_id, peer_id;
                                "snap_key" => ?snap_key,
                                "region_id" => region_id,
                            );
                            should_skip = true;
                        }
                    }
                    MapEntry::Vacant(_) => {
                        // Compat no fast add peer logic
                        // panic!("unknown snapshot!");
                    }
                },
            ).is_err() {
                fatal!("post_apply_snapshot poisoned")
            };
        }

        if should_skip {
            return;
        }

        match self.apply_snap_pool.as_ref() {
            Some(p) => {
                let (sender, receiver) = mpsc::channel();
                let task = Arc::new(PrehandleTask::new(receiver, peer_id));
                {
                    let mut lock = match self.pre_handle_snapshot_ctx.lock() {
                        Ok(l) => l,
                        Err(_) => fatal!("pre_apply_snapshot poisoned"),
                    };
                    let ctx = lock.deref_mut();
                    ctx.tracer.insert(snap_key.clone(), task.clone());
                }

                let engine_store_server_helper = self.engine_store_server_helper;
                let region = ob_region.clone();
                let snap_key = snap_key.clone();
                let ssts = retrieve_sst_files(snap);

                // We use thread pool to do pre handling.
                self.engine
                    .pending_applies_count
                    .fetch_add(1, Ordering::SeqCst);
                p.spawn(async move {
                    // The original implementation is in `Snapshot`, so we don't need to care abort
                    // lifetime.
                    fail::fail_point!("before_actually_pre_handle", |_| {});
                    let res = pre_handle_snapshot_impl(
                        engine_store_server_helper,
                        task.peer_id,
                        ssts,
                        &region,
                        &snap_key,
                    );
                    match sender.send(res) {
                        Err(_e) => {
                            error!("pre apply snapshot err when send to receiver";
                                "region_id" => region.get_id(),
                                "peer_id" => task.peer_id,
                                "snap_key" => ?snap_key,
                            )
                        }
                        Ok(_) => (),
                    }
                });
            }
            None => {
                // quit background pre handling
                warn!("apply_snap_pool is not initialized";
                    "peer_id" => peer_id,
                    "region_id" => region_id
                );
            }
        }
    }

    pub fn post_apply_snapshot(
        &self,
        ob_region: &Region,
        peer_id: u64,
        snap_key: &store::SnapKey,
        snap: Option<&store::Snapshot>,
    ) {
        fail::fail_point!("on_ob_post_apply_snapshot", |_| {
            return;
        });
        let region_id = ob_region.get_id();
        info!("post apply snapshot";
            "peer_id" => ?peer_id,
            "snap_key" => ?snap_key,
            "region_id" => region_id,
            "region" => ?ob_region,
            "pending" => self.engine.pending_applies_count.load(Ordering::SeqCst),
        );
        let mut should_skip = false;
        #[allow(clippy::collapsible_if)]
        if self.packed_envs.engine_store_cfg.enable_fast_add_peer {
            if self.get_cached_manager().access_cached_region_info_mut(
                region_id,
                |info: MapEntry<u64, Arc<CachedRegionInfo>>| match info {
                    MapEntry::Occupied(mut o) => {
                        let is_first_snapsot = !o.get().inited_or_fallback.load(Ordering::SeqCst);
                        if is_first_snapsot {
                            let last = o.get().snapshot_inflight.load(Ordering::SeqCst);
                            let total = o.get().fast_add_peer_start.load(Ordering::SeqCst);
                            let current = SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap();
                            info!("fast path: applied first snapshot {}:{} {}, recover MsgAppend", self.store_id, region_id, peer_id;
                                "snap_key" => ?snap_key,
                                "region_id" => region_id,
                                "cost_snapshot" => current.as_millis() - last,
                                "cost_total" => current.as_millis() - total,
                            );
                            should_skip = true;
                            o.get_mut().snapshot_inflight.store(0, Ordering::SeqCst);
                            o.get_mut().fast_add_peer_start.store(0, Ordering::SeqCst);
                            o.get_mut().inited_or_fallback.store(true, Ordering::SeqCst);
                        }
                    }
                    MapEntry::Vacant(_) => {
                        // Compat no fast add peer logic
                        // panic!("unknown snapshot!");
                    }
                },
            ).is_err() {
                fatal!("post_apply_snapshot poisoned")
            };
        }

        if should_skip {
            return;
        }

        let snap = match snap {
            None => return,
            Some(s) => s,
        };
        let maybe_prehandle_task = {
            let mut lock = match self.pre_handle_snapshot_ctx.lock() {
                Ok(l) => l,
                Err(_) => fatal!("post_apply_snapshot poisoned"),
            };
            let ctx = lock.deref_mut();
            ctx.tracer.remove(snap_key)
        };

        let need_retry = match maybe_prehandle_task {
            Some(t) => {
                let neer_retry = match t.recv.recv() {
                    Ok(snap_ptr) => {
                        info!("get prehandled snapshot success";
                            "peer_id" => peer_id,
                            "snap_key" => ?snap_key,
                            "region_id" => ob_region.get_id(),
                            "pending" => self.engine.pending_applies_count.load(Ordering::SeqCst),
                        );
                        if !should_skip {
                            self.engine_store_server_helper
                                .apply_pre_handled_snapshot(snap_ptr.0);
                        }
                        false
                    }
                    Err(_) => {
                        info!("background pre-handle snapshot get error";
                            "peer_id" => peer_id,
                            "snap_key" => ?snap_key,
                            "region_id" => ob_region.get_id(),
                            "pending" => self.engine.pending_applies_count.load(Ordering::SeqCst),
                        );
                        true
                    }
                };
                // According to pre_apply_snapshot, if registered tracer,
                // then we must have put it into thread pool.
                let _prev = self
                    .engine
                    .pending_applies_count
                    .fetch_sub(1, Ordering::SeqCst);

                #[cfg(any(test, feature = "testexport"))]
                assert!(_prev > 0);

                info!("apply snapshot finished";
                    "peer_id" => peer_id,
                    "snap_key" => ?snap_key,
                    "region" => ?ob_region,
                    "pending" => self.engine.pending_applies_count.load(Ordering::SeqCst),
                );
                neer_retry
            }
            None => {
                // We can't find background pre-handle task, maybe:
                // 1. we can't get snapshot from snap manager at that time.
                // 2. we disabled background pre handling.
                info!("pre-handled snapshot not found";
                    "peer_id" => peer_id,
                    "snap_key" => ?snap_key,
                    "region_id" => ob_region.get_id(),
                    "pending" => self.engine.pending_applies_count.load(Ordering::SeqCst),
                );
                true
            }
        };

        if need_retry && !should_skip {
            // Blocking pre handle.
            let ssts = retrieve_sst_files(snap);
            let ptr = pre_handle_snapshot_impl(
                self.engine_store_server_helper,
                peer_id,
                ssts,
                ob_region,
                snap_key,
            );
            info!("re-gen pre-handled snapshot success";
                "peer_id" => peer_id,
                "snap_key" => ?snap_key,
                "region_id" => ob_region.get_id(),
            );
            self.engine_store_server_helper
                .apply_pre_handled_snapshot(ptr.0);
            info!("apply snapshot finished";
                "peer_id" => peer_id,
                "snap_key" => ?snap_key,
                "region" => ?ob_region,
                "pending" => self.engine.pending_applies_count.load(Ordering::SeqCst),
            );
        }
    }

    pub fn should_pre_apply_snapshot(&self) -> bool {
        true
    }
}

fn retrieve_sst_files(snap: &store::Snapshot) -> Vec<(PathBuf, ColumnFamilyType)> {
    let mut sst_views: Vec<(PathBuf, ColumnFamilyType)> = vec![];
    let mut ssts = vec![];
    for cf_file in snap.cf_files() {
        // Skip empty cf file.
        // CfFile is changed by dynamic region.
        if cf_file.size.is_empty() {
            continue;
        }

        if cf_file.size[0] == 0 {
            continue;
        }

        if plain_file_used(cf_file.cf) {
            assert!(cf_file.cf == CF_LOCK);
        }
        // We have only one file for each cf for now.
        let mut full_paths = cf_file.file_paths();
        assert!(!full_paths.is_empty());
        if full_paths.len() != 1 {
            // Multi sst files for one cf.
            tikv_util::info!("observe multi-file snapshot";
                "snap" => ?snap,
                "cf" => ?cf_file.cf,
                "total" => full_paths.len(),
            );
            for f in full_paths.into_iter() {
                ssts.push((f, name_to_cf(cf_file.cf)));
            }
        } else {
            // Old case, one file for one cf.
            ssts.push((full_paths.remove(0), name_to_cf(cf_file.cf)));
        }
    }
    for (s, cf) in ssts.iter() {
        sst_views.push((PathBuf::from_str(s).unwrap(), *cf));
    }
    sst_views
}

fn pre_handle_snapshot_impl(
    engine_store_server_helper: &'static EngineStoreServerHelper,
    peer_id: u64,
    ssts: Vec<(PathBuf, ColumnFamilyType)>,
    region: &Region,
    snap_key: &SnapKey,
) -> PtrWrapper {
    let idx = snap_key.idx;
    let term = snap_key.term;
    let ptr = {
        let sst_views = ssts
            .iter()
            .map(|(b, c)| (b.to_str().unwrap().as_bytes(), c.clone()))
            .collect();
        engine_store_server_helper.pre_handle_snapshot(region, peer_id, sst_views, idx, term)
    };
    PtrWrapper(ptr)
}
