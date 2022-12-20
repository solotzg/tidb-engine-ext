// Copyright 2016 TiKV Project Authors. Licensed under Apache-2.0.

use engine_traits::{Engines, KvEngine, Mutable, RaftEngine, WriteBatch, RaftLogBatch, CF_DEFAULT};
use kvproto::{
    metapb,
    raft_serverpb::{RaftLocalState, RegionLocalState, StoreIdent},
};
use tikv_util::{box_err, box_try, store::new_peer};

use super::peer_storage::{
    write_initial_apply_state, write_initial_raft_state, INIT_EPOCH_CONF_VER, INIT_EPOCH_VER,
};
use crate::Result;

pub fn initial_region(store_id: u64, region_id: u64, peer_id: u64) -> metapb::Region {
    let mut region = metapb::Region::default();
    region.set_id(region_id);
    region.set_start_key(keys::EMPTY_KEY.to_vec());
    region.set_end_key(keys::EMPTY_KEY.to_vec());
    region.mut_region_epoch().set_version(INIT_EPOCH_VER);
    region.mut_region_epoch().set_conf_ver(INIT_EPOCH_CONF_VER);
    region.mut_peers().push(new_peer(store_id, peer_id));
    region
}

// check no any data in range [start_key, end_key)
fn is_range_empty(
    engine: &impl KvEngine,
    cf: &str,
    start_key: &[u8],
    end_key: &[u8],
) -> Result<bool> {
    let mut count: u32 = 0;
    engine.scan(cf, start_key, end_key, false, |_, _| {
        count += 1;
        Ok(false)
    })?;

    Ok(count == 0)
}

// Bootstrap the store, the DB for this store must be empty and has no data.
//
// FIXME: ER typaram should just be impl KvEngine, but RaftEngine doesn't
// support the `is_range_empty` query yet.
pub fn bootstrap_store<ER>(
    engines: &Engines<impl KvEngine, ER>,
    cluster_id: u64,
    store_id: u64,
) -> Result<()>
where
    ER: RaftEngine,
{
    let mut ident = StoreIdent::default();

    if !is_range_empty(&engines.kv, CF_DEFAULT, keys::MIN_KEY, keys::MAX_KEY)? {
        return Err(box_err!("kv store is not empty and has already had data."));
    }

    ident.set_cluster_id(cluster_id);
    ident.set_store_id(store_id);

    engines.raft.put_store_ident(&ident)?;
    Ok(())
}

/// The first phase of bootstrap cluster
///
/// Write the first region meta and prepare state.
pub fn prepare_bootstrap_cluster(
    engines: &Engines<impl KvEngine, impl RaftEngine>,
    region: &metapb::Region,
) -> Result<()> {
    let mut state = RegionLocalState::default();
    state.set_region(region.clone());

    let mut raft_wb = engines.raft.log_batch(1024);
    box_try!(raft_wb.put_prepare_bootstrap_region(region));
    box_try!(raft_wb.put_region_state(region.get_id(), &state));
    write_initial_apply_state(&mut raft_wb, region.get_id())?;

    write_initial_raft_state(&mut raft_wb, region.get_id())?;
    box_try!(engines.raft.consume(&mut raft_wb, true));
    Ok(())
}

// Clear first region meta and prepare key.
pub fn clear_prepare_bootstrap_cluster(
    engines: &Engines<impl KvEngine, impl RaftEngine>,
    region_id: u64,
) -> Result<()> {
    let mut wb = engines.raft.log_batch(1024);
    box_try!(
        engines
            .raft
            .clean(region_id, 0, &RaftLocalState::default(), &mut wb)
    );
    wb.remove_prepare_bootstrap_region();
    wb.remove_region_state(region_id);
    wb.remove_apply_state(region_id);
    box_try!(engines.raft.consume(&mut wb, true));

    Ok(())
}

// Clear prepare key
pub fn clear_prepare_bootstrap_key(
    engines: &Engines<impl KvEngine, impl RaftEngine>,
) -> Result<()> {
    let mut raft_wb = engines.raft.log_batch(1024);
    raft_wb.remove_prepare_bootstrap_region();
    engines.raft.consume(&mut raft_wb, true);
    Ok(())
}

#[cfg(test)]
mod tests {
    use engine_traits::{
        Engines, Peekable, RaftEngineDebug, RaftEngineReadOnly, RaftLogBatch, CF_DEFAULT,
    };
    use tempfile::Builder;

    use super::*;

    #[test]
    fn test_bootstrap() {
        let path = Builder::new().prefix("var").tempdir().unwrap();
        let raft_path = path.path().join("raft");
        let kv_engine =
            engine_test::kv::new_engine(path.path().to_str().unwrap(), &[CF_DEFAULT])
                .unwrap();
        let raft_engine = engine_test::raft::new_engine(raft_path.to_str().unwrap(), None).unwrap();
        let engines = Engines::new(kv_engine.clone(), raft_engine.clone());
        let region = initial_region(1, 1, 1);

        bootstrap_store(&engines, 1, 1).unwrap();
        bootstrap_store(&engines, 1, 1).unwrap_err();

        prepare_bootstrap_cluster(&engines, &region).unwrap();
        assert!(
            raft_engine
                .get_prepare_bootstrap_region()
                .unwrap()
                .is_some()
        );
        assert!(
            raft_engine
                .get_region_state(1)
                .unwrap()
                .is_some()
        );
        assert!(
            raft_engine
                .get_apply_state(1)
                .unwrap()
                .is_some()
        );
        assert!(raft_engine.get_raft_state(1).unwrap().is_some());

        clear_prepare_bootstrap_key(&engines).unwrap();
        clear_prepare_bootstrap_cluster(&engines, 1).unwrap();
        assert!(RaftLogBatch::is_empty(&raft_engine.dump_all_data(1)));
    }
}
