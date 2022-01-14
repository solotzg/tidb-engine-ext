use raftstore::engine_store_ffi::interfaces::root::DB::RawRustPtr;
use raftstore::engine_store_ffi::{
    ffi_gc_rust_ptr, ffi_make_async_waker, ffi_make_read_index_task, ffi_poll_read_index_task,
    ProtoMsgBaseBuff, RawVoidPtr,
};
use std::collections::hash_map::Entry;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use test_raftstore::*;

#[derive(Default)]
struct GcMonitor {
    data: Mutex<collections::HashMap<u32, isize>>,
}

impl GcMonitor {
    fn add(&self, ptr: &RawRustPtr, x: isize) {
        let data = &mut *self.data.lock().unwrap();
        match data.entry(ptr.type_) {
            Entry::Occupied(mut v) => {
                *v.get_mut() += x;
            }
            Entry::Vacant(v) => {
                v.insert(x);
            }
        }
    }
    fn valid_clean(&self) -> bool {
        let data = &*self.data.lock().unwrap();
        for (_, v) in data {
            if *v != 0 {
                return false;
            }
        }
        return true;
    }
    fn is_empty(&self) -> bool {
        let data = &*self.data.lock().unwrap();
        data.is_empty()
    }
}

lazy_static! {
    static ref GC_MONITOR: GcMonitor = GcMonitor::default();
}

struct RawRustPtrWrap(RawRustPtr);

impl Drop for RawRustPtrWrap {
    fn drop(&mut self) {
        ffi_gc_rust_ptr(self.0.ptr, self.0.type_);
        GC_MONITOR.add(&self.0, -1);
    }
}

struct ReadIndexFutureTask {
    ptr: RawRustPtrWrap,
}

struct Waker {
    _inner: RawRustPtrWrap,
    notifier: RawVoidPtr,
}

impl Waker {
    pub fn new() -> Self {
        let notifier = mock_engine_store::ProxyNotifier::new_raw();
        let ptr = notifier.ptr;
        let notifier = ffi_make_async_waker(Some(ffi_wake), notifier);
        GC_MONITOR.add(&notifier, 1);
        Self {
            _inner: RawRustPtrWrap(notifier),
            notifier: ptr,
        }
    }

    fn wait_for(&self, timeout: Duration) {
        // Block wait for test
        self.get_notifier().blocked_wait_for(timeout)
    }

    fn get_notifier(&self) -> &mock_engine_store::ProxyNotifier {
        unsafe { &*(self.notifier as *mut mock_engine_store::ProxyNotifier) }
    }

    fn get_raw_waker(&self) -> RawVoidPtr {
        self._inner.0.ptr
    }
}

fn single_blocked_read_index(
    req: &kvproto::kvrpcpb::ReadIndexRequest,
    ffi_helper: &raftstore::engine_store_ffi::RaftStoreProxyFFIHelper,
    waker: Option<&Waker>,
) -> Option<kvproto::kvrpcpb::ReadIndexResponse> {
    let req = ProtoMsgBaseBuff::new(req);
    let mut ptr = RawRustPtr {
        ptr: std::ptr::null_mut(),
        type_: 0,
    };
    let mut tasks = std::collections::LinkedList::new();
    let mut resp = kvproto::kvrpcpb::ReadIndexResponse::default();

    if 0 != ffi_make_read_index_task(ffi_helper.proxy_ptr, Pin::new(&req).into(), &mut ptr) {
        GC_MONITOR.add(&ptr, 1);
        tasks.push_back(ReadIndexFutureTask {
            ptr: RawRustPtrWrap(ptr),
        });
    } else {
        return None;
    }
    while !tasks.is_empty() {
        let t = tasks.front().unwrap();
        let waker_ptr = match waker {
            None => std::ptr::null_mut(),
            Some(w) => w.get_raw_waker(),
        };
        if 0 != ffi_poll_read_index_task(
            ffi_helper.proxy_ptr,
            t.ptr.0.ptr,
            &mut resp as *mut _ as RawVoidPtr,
            waker_ptr,
        ) {
            tasks.pop_front().unwrap();
        } else {
            if let Some(w) = waker {
                w.wait_for(Duration::from_secs(5));
            } else {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
    Some(resp)
}

#[no_mangle]
extern "C" fn ffi_wake(data: RawVoidPtr) {
    let notifier = unsafe { &mut *(data as *mut mock_engine_store::ProxyNotifier) };
    notifier.wake()
}

#[test]
fn test_duplicate_read_index_ctx() {
    // Initialize cluster
    let mut cluster = new_node_cluster(0, 3);
    configure_for_lease_read(&mut cluster, Some(50), Some(10_000));
    cluster.cfg.raft_store.raft_heartbeat_ticks = 1;
    let pd_client = Arc::clone(&cluster.pd_client);
    pd_client.disable_default_operator();

    // Set region and peers
    let r1 = cluster.run_conf_change();
    let p1 = new_peer(1, 1);
    let p2 = new_peer(2, 2);
    cluster.pd_client.must_add_peer(r1, p2.clone());
    let p3 = new_peer(3, 3);
    cluster.pd_client.must_add_peer(r1, p3.clone());
    cluster.must_put(b"k0", b"v0");
    cluster.pd_client.must_none_pending_peer(p2.clone());
    cluster.pd_client.must_none_pending_peer(p3.clone());
    let region = cluster.get_region(b"k0");
    assert_eq!(cluster.leader_of_region(region.get_id()).unwrap(), p1);

    let waker = Waker::new();

    for (id, peer, f) in &[(2, p2, true), (3, p3, false)] {
        let ffi_helper = cluster.ffi_helper_set.get(id).unwrap();
        let mut request = kvproto::kvrpcpb::ReadIndexRequest::default();

        {
            let context = request.mut_context();
            context.set_region_id(region.get_id());
            context.set_peer(peer.clone());
            context.set_region_epoch(region.get_region_epoch().clone());
        }
        let w = if *f { Some(&waker) } else { None };
        let resp = single_blocked_read_index(&request, &*ffi_helper.proxy_helper, w).unwrap();
        assert!(resp.get_read_index() != 0);
        assert!(!resp.has_region_error());
        assert!(!resp.has_locked());
    }

    drop(waker);

    {
        assert!(!GC_MONITOR.is_empty());
        assert!(GC_MONITOR.valid_clean());
    }
}
