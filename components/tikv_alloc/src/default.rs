// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

// The implementation of this crate when jemalloc is turned on

use std::{
    collections::HashMap,
    ptr::{self, NonNull},
    slice,
    sync::Mutex,
    thread,
};

use libc::{self, c_char, c_void};

use super::error::{ProfError, ProfResult};
use crate::AllocStats;

lazy_static! {
    static ref THREAD_MEMORY_MAP: Mutex<HashMap<ThreadId, MemoryStatsAccessor>> =
        Mutex::new(HashMap::new());
}

/// The struct for tracing the statistic of another thread.
/// The target pointer should be bound to some TLS of another thread, this
/// structure is just "peeking" it -- with out modifying.
// It should be covariant so we wrap it with `NonNull`.
#[repr(transparent)]
struct PeekableRemoteStat<T>(Option<NonNull<T>>);

// SAFETY: all constructors of `PeekableRemoteStat` returns pointer points to a
// thread local variable. Once this be sent, a reasonable life time of this
// variable should be as long as the thread holding the underlying thread local
// variable. But it is impossible to express such lifetime in current Rust.
// Then it is the user's responsibility to trace that lifetime.
unsafe impl<T: Send> Send for PeekableRemoteStat<T> {}

impl<T: Copy> PeekableRemoteStat<T> {
    /// Try access the underlying data. When the pointer is `nullptr`, returns
    /// `None`.
    ///
    /// # Safety
    ///
    /// The pointer should not be dangling. (i.e. the thread to be traced should
    /// be accessible.)
    unsafe fn peek(&self) -> Option<T> {
        self.0
            .map(|nlp| unsafe { core::intrinsics::atomic_load_seqcst(nlp.as_ptr()) })
    }

    fn from_raw(ptr: *mut T) -> Self {
        Self(NonNull::new(ptr))
    }
}

impl PeekableRemoteStat<u64> {
    fn allocated() -> Self {
        // SAFETY: it is transparent.
        // NOTE: perhaps we'd better add something like `as_raw()` for `ThreadLocal`...
        Self::from_raw(
            tikv_jemalloc_ctl::thread::allocatedp::read()
                .map(|x| unsafe { std::mem::transmute(x) })
                .unwrap_or(std::ptr::null_mut()),
        )
    }

    fn deallocated() -> Self {
        // SAFETY: it is transparent.
        Self::from_raw(
            tikv_jemalloc_ctl::thread::deallocatedp::read()
                .map(|x| unsafe { std::mem::transmute(x) })
                .unwrap_or(std::ptr::null_mut()),
        )
    }
}

struct MemoryStatsAccessor {
    allocated: PeekableRemoteStat<u64>,
    deallocated: PeekableRemoteStat<u64>,
    thread_name: String,
}

impl MemoryStatsAccessor {
    fn get_allocated(&self) -> u64 {
        // SAFETY: `add_thread_memory_accessor` is unsafe, and that is the only way for
        // outer crates to create this.
        unsafe { self.allocated.peek().unwrap_or_default() }
    }

    fn get_deallocated(&self) -> u64 {
        // SAFETY: `add_thread_memory_accessor` is unsafe, and that is the only way for
        // outer crates to create this.
        unsafe { self.deallocated.peek().unwrap_or_default() }
    }
}

/// Register the current thread to the collector that collects the jemalloc
/// allocation / deallocation info.
///
/// Generally you should call this via `spawn_wrapper`s instead of invoke this
/// directly. The former is a safe function.
///
/// # Safety
///
/// Make sure the `remove_thread_memory_accessor` is called before the thread
/// exits.
pub unsafe fn add_thread_memory_accessor() {
    let mut thread_memory_map = THREAD_MEMORY_MAP.lock().unwrap();
    thread_memory_map
        .entry(thread::current().id())
        .or_insert_with(|| {
            let allocated = PeekableRemoteStat::allocated();
            let deallocated = PeekableRemoteStat::deallocated();

            MemoryStatsAccessor {
                thread_name: thread::current().name().unwrap_or("<unknown>").to_string(),
                allocated,
                deallocated,
            }
        });
}

pub fn remove_thread_memory_accessor() {
    let mut thread_memory_map = THREAD_MEMORY_MAP.lock().unwrap();
    thread_memory_map.remove(&thread::current().id());
}

use std::thread::ThreadId;

pub fn dump_stats() -> String {
    let mut buf = Vec::with_capacity(1024);

    unsafe {
        malloc_stats_print(
            Some(write_cb),
            &mut buf as *mut Vec<u8> as *mut c_void,
            ptr::null(),
        );
    }
    let mut memory_stats = format!(
        "Memory stats summary: {}\n",
        String::from_utf8_lossy(&buf).into_owned()
    );
    memory_stats.push_str("Memory stats by thread:\n");

    let thread_memory_map = THREAD_MEMORY_MAP.lock().unwrap();
    for (_, accessor) in thread_memory_map.iter() {
        let alloc = accessor.get_allocated();
        let dealloc = accessor.get_deallocated();
        memory_stats.push_str(
            format!(
                "Thread [{}]: alloc_bytes={alloc},dealloc_bytes={dealloc}\n",
                accessor.thread_name
            )
            .as_str(),
        );
    }
    memory_stats
}

pub fn fetch_stats() -> Result<Option<AllocStats>, Error> {
    // Stats are cached. Need to advance epoch to refresh.
    epoch::advance()?;

    Ok(Some(vec![
        ("allocated", stats::allocated::read()?),
        ("active", stats::active::read()?),
        ("metadata", stats::metadata::read()?),
        ("resident", stats::resident::read()?),
        ("mapped", stats::mapped::read()?),
        ("retained", stats::retained::read()?),
        (
            "dirty",
            stats::resident::read()? - stats::active::read()? - stats::metadata::read()?,
        ),
        (
            "fragmentation",
            stats::active::read()? - stats::allocated::read()?,
        ),
    ]))
}

/// remove the postfix of threads generated by the YATP (-*).
/// YATP will append the id of the threads in a thread pool, which will bring
/// too many labels to the metric (and usually the memory usage should be evenly
/// distributed among these threads).
/// Fine-grained memory statistic is still available in the interface provided
/// for `tikv-ctl`.
fn trim_yatp_suffix(s: &str) -> &str {
    s.trim_end_matches(|c: char| c.is_ascii_digit() || c == '-')
}

/// Iterate over the allocation stat.
/// Format of the callback: `(name, allocated, deallocated)`.
pub fn iterate_thread_allocation_stats(mut f: impl FnMut(&str, u64, u64)) {
    // Given we have called `epoch::advance()` in `fetch_stats`, we (magically!)
    // skip advancing the epoch here.
    let thread_memory_map = THREAD_MEMORY_MAP.lock().unwrap();
    let mut collected = HashMap::<&str, (u64, u64)>::with_capacity(thread_memory_map.len());
    for (_, accessor) in thread_memory_map.iter() {
        let ent = collected
            .entry(trim_yatp_suffix(&accessor.thread_name))
            .or_default();
        ent.0 += accessor.get_allocated();
        ent.1 += accessor.get_deallocated();
    }
    for (name, val) in collected {
        f(name, val.0, val.1)
    }
}

#[allow(clippy::cast_ptr_alignment)]
extern "C" fn write_cb(printer: *mut c_void, msg: *const c_char) {
    unsafe {
        // This cast from *c_void to *Vec<u8> looks like a bad
        // cast to clippy due to pointer alignment, but we know
        // what type the pointer is.
        let buf = &mut *(printer as *mut Vec<u8>);
        let len = libc::strlen(msg);
        let bytes = slice::from_raw_parts(msg as *const u8, len);
        buf.extend_from_slice(bytes);
    }
}
