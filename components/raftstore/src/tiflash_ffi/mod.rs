use encryption::DataKeyManager;
use engine_rocks::encryption::get_env;
use engine_rocks::RocksSstReader;
use engine_traits::{
    EncryptionKeyManager, EncryptionMethod, FileEncryptionInfo, Iterator, SeekKey, SstReader,
    CF_DEFAULT, CF_LOCK, CF_WRITE,
};
use kvproto::{metapb, raft_cmdpb};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

type TiFlashServerPtr = *const u8;
type RegionId = u64;
pub type SnapshotKV = VecDeque<(Vec<u8>, Vec<u8>)>;
pub type SnapshotKVView = (Vec<BaseBuffView>, Vec<BaseBuffView>);

pub enum TiFlashApplyRes {
    None,
    Persist,
    NotFound,
}

impl From<u32> for TiFlashApplyRes {
    fn from(o: u32) -> Self {
        match o {
            0 => TiFlashApplyRes::None,
            1 => TiFlashApplyRes::Persist,
            2 => TiFlashApplyRes::NotFound,
            _ => unreachable!(),
        }
    }
}

pub struct TiFlashRaftProxy {
    pub stopped: AtomicBool,
    pub key_manager: Option<Arc<DataKeyManager>>,
}

type TiFlashRaftProxyPtr = *const TiFlashRaftProxy;

#[no_mangle]
pub extern "C" fn ffi_handle_check_stopped(proxy_ptr: TiFlashRaftProxyPtr) -> u8 {
    unsafe { (*proxy_ptr).stopped.load(Ordering::SeqCst) as u8 }
}

#[no_mangle]
pub extern "C" fn ffi_is_encryption_enabled(proxy_ptr: TiFlashRaftProxyPtr) -> u8 {
    unsafe { (*proxy_ptr).key_manager.is_some().into() }
}

#[no_mangle]
pub extern "C" fn ffi_encryption_method(proxy_ptr: TiFlashRaftProxyPtr) -> u8 {
    unsafe {
        (*proxy_ptr)
            .key_manager
            .as_ref()
            .map_or(EncryptionMethod::Plaintext, |x| x.encryption_method()) as u8
    }
}

enum FileEncryptionRes {
    Disabled,
    Ok,
    Error,
}

impl Into<u8> for FileEncryptionRes {
    fn into(self) -> u8 {
        return match self {
            FileEncryptionRes::Disabled => 0,
            FileEncryptionRes::Ok => 1,
            FileEncryptionRes::Error => 2,
        };
    }
}

type TiFlashRawString = *const u8;

#[repr(C)]
pub struct FileEncryptionInfoRes {
    pub res: u8,
    pub method: u8,
    pub key: TiFlashRawString,
    pub iv: TiFlashRawString,
    pub erro_msg: TiFlashRawString,
}

impl FileEncryptionInfoRes {
    fn new(res: FileEncryptionRes) -> Self {
        FileEncryptionInfoRes {
            res: res.into(),
            method: EncryptionMethod::Unknown as u8,
            key: std::ptr::null(),
            iv: std::ptr::null(),
            erro_msg: std::ptr::null(),
        }
    }

    fn error(erro_msg: TiFlashRawString) -> Self {
        FileEncryptionInfoRes {
            res: FileEncryptionRes::Error.into(),
            method: EncryptionMethod::Unknown as u8,
            key: std::ptr::null(),
            iv: std::ptr::null(),
            erro_msg,
        }
    }

    fn from(f: FileEncryptionInfo) -> Self {
        FileEncryptionInfoRes {
            res: FileEncryptionRes::Ok.into(),
            method: f.method as u8,
            key: get_tiflash_server_helper().gen_cpp_string(&f.key),
            iv: get_tiflash_server_helper().gen_cpp_string(&f.iv),
            erro_msg: std::ptr::null(),
        }
    }
}

#[no_mangle]
pub extern "C" fn ffi_handle_get_file(
    proxy_ptr: TiFlashRaftProxyPtr,
    name: BaseBuffView,
) -> FileEncryptionInfoRes {
    unsafe {
        (*proxy_ptr).key_manager.as_ref().map_or(
            FileEncryptionInfoRes::new(FileEncryptionRes::Disabled),
            |key_manager| {
                let p = key_manager.get_file(std::str::from_utf8_unchecked(name.to_slice()));
                p.map_or_else(
                    |e| {
                        FileEncryptionInfoRes::error(get_tiflash_server_helper().gen_cpp_string(
                            format!("Encryption key manager get file failure: {}", e).as_ref(),
                        ))
                    },
                    |f| FileEncryptionInfoRes::from(f),
                )
            },
        )
    }
}

#[no_mangle]
pub extern "C" fn ffi_handle_new_file(
    proxy_ptr: TiFlashRaftProxyPtr,
    name: BaseBuffView,
) -> FileEncryptionInfoRes {
    unsafe {
        (*proxy_ptr).key_manager.as_ref().map_or(
            FileEncryptionInfoRes::new(FileEncryptionRes::Disabled),
            |key_manager| {
                let p = key_manager.new_file(std::str::from_utf8_unchecked(name.to_slice()));
                p.map_or_else(
                    |e| {
                        FileEncryptionInfoRes::error(get_tiflash_server_helper().gen_cpp_string(
                            format!("Encryption key manager new file failure: {}", e).as_ref(),
                        ))
                    },
                    |f| FileEncryptionInfoRes::from(f),
                )
            },
        )
    }
}

#[no_mangle]
pub extern "C" fn ffi_handle_delete_file(
    proxy_ptr: TiFlashRaftProxyPtr,
    name: BaseBuffView,
) -> FileEncryptionInfoRes {
    unsafe {
        (*proxy_ptr).key_manager.as_ref().map_or(
            FileEncryptionInfoRes::new(FileEncryptionRes::Disabled),
            |key_manager| {
                let p = key_manager.delete_file(std::str::from_utf8_unchecked(name.to_slice()));
                p.map_or_else(
                    |e| {
                        FileEncryptionInfoRes::error(get_tiflash_server_helper().gen_cpp_string(
                            format!("Encryption key manager delete file failure: {}", e).as_ref(),
                        ))
                    },
                    |_| FileEncryptionInfoRes::new(FileEncryptionRes::Ok),
                )
            },
        )
    }
}

#[no_mangle]
pub extern "C" fn ffi_handle_link_file(
    proxy_ptr: TiFlashRaftProxyPtr,
    src: BaseBuffView,
    dst: BaseBuffView,
) -> FileEncryptionInfoRes {
    unsafe {
        (*proxy_ptr).key_manager.as_ref().map_or(
            FileEncryptionInfoRes::new(FileEncryptionRes::Disabled),
            |key_manager| {
                let p = key_manager.link_file(
                    std::str::from_utf8_unchecked(src.to_slice()),
                    std::str::from_utf8_unchecked(dst.to_slice()),
                );
                p.map_or_else(
                    |e| {
                        FileEncryptionInfoRes::error(get_tiflash_server_helper().gen_cpp_string(
                            format!("Encryption key manager link file failure: {}", e).as_ref(),
                        ))
                    },
                    |_| FileEncryptionInfoRes::new(FileEncryptionRes::Ok),
                )
            },
        )
    }
}

#[no_mangle]
pub extern "C" fn ffi_handle_rename_file(
    proxy_ptr: TiFlashRaftProxyPtr,
    src: BaseBuffView,
    dst: BaseBuffView,
) -> FileEncryptionInfoRes {
    unsafe {
        (*proxy_ptr).key_manager.as_ref().map_or(
            FileEncryptionInfoRes::new(FileEncryptionRes::Disabled),
            |key_manager| {
                let p = key_manager.rename_file(
                    std::str::from_utf8_unchecked(src.to_slice()),
                    std::str::from_utf8_unchecked(dst.to_slice()),
                );
                p.map_or_else(
                    |e| {
                        FileEncryptionInfoRes::error(get_tiflash_server_helper().gen_cpp_string(
                            format!("Encryption key manager rename file failure: {}", e).as_ref(),
                        ))
                    },
                    |_| FileEncryptionInfoRes::new(FileEncryptionRes::Ok),
                )
            },
        )
    }
}

#[repr(C)]
pub struct TiFlashRaftProxyHelper {
    proxy_ptr: TiFlashRaftProxyPtr,
    handle_check_stopped: extern "C" fn(TiFlashRaftProxyPtr) -> u8,
    is_encryption_enabled: extern "C" fn(TiFlashRaftProxyPtr) -> u8,
    encryption_method: extern "C" fn(TiFlashRaftProxyPtr) -> u8,
    handle_get_file: extern "C" fn(TiFlashRaftProxyPtr, BaseBuffView) -> FileEncryptionInfoRes,
    handle_new_file: extern "C" fn(TiFlashRaftProxyPtr, BaseBuffView) -> FileEncryptionInfoRes,
    handle_delete_file: extern "C" fn(TiFlashRaftProxyPtr, BaseBuffView) -> FileEncryptionInfoRes,
    handle_link_file:
        extern "C" fn(TiFlashRaftProxyPtr, BaseBuffView, BaseBuffView) -> FileEncryptionInfoRes,
    handle_rename_file:
        extern "C" fn(TiFlashRaftProxyPtr, BaseBuffView, BaseBuffView) -> FileEncryptionInfoRes,
}

impl TiFlashRaftProxyHelper {
    pub fn new(proxy: &TiFlashRaftProxy) -> Self {
        TiFlashRaftProxyHelper {
            proxy_ptr: proxy,
            handle_check_stopped: ffi_handle_check_stopped,
            is_encryption_enabled: ffi_is_encryption_enabled,
            encryption_method: ffi_encryption_method,
            handle_get_file: ffi_handle_get_file,
            handle_new_file: ffi_handle_new_file,
            handle_delete_file: ffi_handle_delete_file,
            handle_link_file: ffi_handle_link_file,
            handle_rename_file: ffi_handle_rename_file,
        }
    }
}

pub fn gen_snap_kv_data_from_sst(
    cf_file_path: &str,
    key_manager: Option<Arc<DataKeyManager>>,
) -> SnapshotKV {
    let mut cf_snap = SnapshotKV::new();
    let env = get_env(key_manager, None).unwrap();
    let sst_reader = RocksSstReader::open_with_env(cf_file_path, Some(env)).unwrap();
    sst_reader.verify_checksum().unwrap();
    let mut iter = sst_reader.iter();
    let mut remained = iter.seek(SeekKey::Start).unwrap();
    while remained {
        let ori_key = keys::origin_key(iter.key());
        let ori_val = iter.value();
        cf_snap.push_back((ori_key.to_vec(), ori_val.to_vec()));
        remained = iter.next().unwrap();
    }

    cf_snap
}

pub enum WriteCmdType {
    Put,
    Del,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum WriteCmdCf {
    Lock,
    Write,
    Default,
}

pub fn name_to_cf(cf: &str) -> WriteCmdCf {
    if cf.is_empty() {
        return WriteCmdCf::Default;
    }
    if cf == CF_LOCK {
        return WriteCmdCf::Lock;
    } else if cf == CF_WRITE {
        return WriteCmdCf::Write;
    } else if cf == CF_DEFAULT {
        return WriteCmdCf::Default;
    }
    unreachable!()
}

#[repr(C)]
pub struct WriteCmdsView {
    keys: *const BaseBuffView,
    vals: *const BaseBuffView,
    cmd_types: *const u8,
    cf: *const u8,
    len: u64,
}

impl Into<u8> for WriteCmdType {
    fn into(self) -> u8 {
        return match self {
            WriteCmdType::Put => 0,
            WriteCmdType::Del => 1,
        };
    }
}

impl Into<u8> for WriteCmdCf {
    fn into(self) -> u8 {
        return match self {
            WriteCmdCf::Lock => 0,
            WriteCmdCf::Write => 1,
            WriteCmdCf::Default => 2,
        };
    }
}

#[derive(Default)]
pub struct WriteCmds {
    keys: Vec<BaseBuffView>,
    vals: Vec<BaseBuffView>,
    cmd_type: Vec<u8>,
    cf: Vec<u8>,
}

impl WriteCmds {
    pub fn with_capacity(cap: usize) -> WriteCmds {
        WriteCmds {
            keys: Vec::<BaseBuffView>::with_capacity(cap),
            vals: Vec::<BaseBuffView>::with_capacity(cap),
            cmd_type: Vec::<u8>::with_capacity(cap),
            cf: Vec::<u8>::with_capacity(cap),
        }
    }

    pub fn new() -> WriteCmds {
        WriteCmds::default()
    }

    pub fn push(&mut self, key: &[u8], val: &[u8], cmd_type: WriteCmdType, cf: u8) {
        self.keys.push(BaseBuffView {
            data: key.as_ptr(),
            len: key.len() as u64,
        });
        self.vals.push(BaseBuffView {
            data: val.as_ptr(),
            len: val.len() as u64,
        });
        self.cmd_type.push(cmd_type.into());
        self.cf.push(cf);
    }

    pub fn len(&self) -> usize {
        return self.cmd_type.len();
    }

    fn gen_view(&self) -> WriteCmdsView {
        WriteCmdsView {
            keys: self.keys.as_ptr(),
            vals: self.vals.as_ptr(),
            cmd_types: self.cmd_type.as_ptr(),
            cf: self.cf.as_ptr(),
            len: self.cmd_type.len() as u64,
        }
    }
}

pub fn gen_snap_kv_data_view(snap: &SnapshotKV) -> SnapshotKVView {
    let mut keys = Vec::<BaseBuffView>::with_capacity(snap.len());
    let mut vals = Vec::<BaseBuffView>::with_capacity(snap.len());

    for (k, v) in snap {
        keys.push(BaseBuffView {
            data: k.as_ptr(),
            len: k.len() as u64,
        });
        vals.push(BaseBuffView {
            data: v.as_ptr(),
            len: v.len() as u64,
        });
    }

    (keys, vals)
}

#[repr(C)]
pub struct SnapshotView {
    keys: *const BaseBuffView,
    vals: *const BaseBuffView,
    cf: u8,
    len: u64,
}

#[repr(C)]
struct SnapshotViewArray {
    views: *const SnapshotView,
    len: u64,
}

#[derive(Default)]
pub struct SnapshotHelper {
    cf_snaps: Vec<(WriteCmdCf, SnapshotKV)>,
    kv_view: Vec<SnapshotKVView>,
    snap_view: Vec<SnapshotView>,
}

impl SnapshotHelper {
    pub fn add_cf_snap(&mut self, cf_type: WriteCmdCf, snap_kv: SnapshotKV) {
        self.cf_snaps.push((cf_type, snap_kv));
    }

    fn gen_snapshot_view(&mut self) -> SnapshotViewArray {
        let len = self.cf_snaps.len();
        self.kv_view.clear();
        self.snap_view.clear();

        for i in 0..len {
            self.kv_view
                .push(gen_snap_kv_data_view(&self.cf_snaps[i].1));
        }

        for i in 0..len {
            self.snap_view.push(SnapshotView {
                keys: self.kv_view[i].0.as_ptr(),
                vals: self.kv_view[i].1.as_ptr(),
                len: self.kv_view[i].0.len() as u64,
                cf: self.cf_snaps[i].0.clone().into(),
            });
        }
        SnapshotViewArray {
            views: self.snap_view.as_ptr(),
            len: self.snap_view.len() as u64,
        }
    }

    pub fn empty(&self) -> bool {
        self.cf_snaps.len() == 0
    }
}

#[repr(C)]
pub struct BaseBuffView {
    data: *const u8,
    len: u64,
}

impl BaseBuffView {
    pub fn to_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.data, self.len as usize) }
    }
}

impl From<&[u8]> for BaseBuffView {
    fn from(s: &[u8]) -> Self {
        Self {
            data: s.as_ptr(),
            len: s.len() as u64,
        }
    }
}

impl Default for BaseBuffView {
    fn default() -> Self {
        Self {
            data: std::ptr::null(),
            len: 0,
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct RaftCmdHeader {
    region_id: u64,
    index: u64,
    term: u64,
}

impl RaftCmdHeader {
    pub fn new(region_id: u64, index: u64, term: u64) -> Self {
        RaftCmdHeader {
            region_id,
            index,
            term,
        }
    }
}

struct ProtoMsgBaseBuff {
    _data: Vec<u8>,
    buff_view: BaseBuffView,
}

impl ProtoMsgBaseBuff {
    fn new<T: protobuf::Message>(msg: &T) -> Self {
        let v = msg.write_to_bytes().unwrap();
        let ptr = v.as_ptr();
        let len = v.len() as u64;
        ProtoMsgBaseBuff {
            _data: v,
            buff_view: BaseBuffView { data: ptr, len },
        }
    }
}

#[repr(C)]
pub struct FsStats {
    pub used_size: u64,
    pub avail_size: u64,
    pub capacity_size: u64,
    pub ok: u8,
}

#[repr(C)]
pub struct CppStrWithView {
    inner: RawCppPtr,
    pub view: BaseBuffView,
}

impl CppStrWithView {
    fn is_none(&self) -> bool {
        self.inner.ptr == std::ptr::null()
    }
}

#[repr(C)]
pub struct RawCppPtr {
    ptr: *const u8,
    tp: u32,
}

impl RawCppPtr {
    pub fn raw_ptr(&self) -> *const u8 {
        self.ptr
    }

    pub fn get_type(&self) -> u32 {
        self.tp
    }

    fn into_raw(mut self) -> *const u8 {
        let ptr = self.ptr;
        self.ptr = std::ptr::null();
        ptr
    }

    pub fn is_null(&self) -> bool {
        self.ptr == std::ptr::null()
    }
}

impl Drop for RawCppPtr {
    fn drop(&mut self) {
        if !self.is_null() {
            get_tiflash_server_helper().gc_raw_cpp_ptr(RawCppPtr {
                ptr: self.ptr,
                tp: self.tp,
            });
            self.ptr = std::ptr::null();
        }
    }
}

unsafe impl Send for RawCppPtr {}

#[repr(C)]
pub struct SerializeTiFlashSnapshotRes {
    pub ok: u8,
    pub key_count: u64,
    pub total_size: u64,
}

#[repr(C)]
pub struct GetRegionApproximateSizeKeysRes {
    pub ok: u8,
    pub size: u64,
    pub keys: u64,
}

#[repr(C)]
pub struct SplitKeysWithView {
    inner: RawCppPtr,
    view: *const BaseBuffView,
    len: u64,
}

impl Into<Vec<Vec<u8>>> for SplitKeysWithView {
    fn into(self) -> Vec<Vec<u8>> {
        let mut res = vec![];
        for i in 0..self.len {
            unsafe { res.push((*self.view.offset(i as isize)).to_slice().to_vec()) }
        }
        res
    }
}

#[repr(C)]
pub struct ScanSplitKeysRes {
    pub ok: u8,
    pub size: u64,
    pub keys: u64,
    pub split_keys: SplitKeysWithView,
}

#[repr(C)]
pub struct TiFlashServerHelper {
    magic_number: u32,
    version: u32,
    //
    inner: TiFlashServerPtr,
    gen_cpp_string: extern "C" fn(BaseBuffView) -> RawCppPtr,
    handle_write_raft_cmd: extern "C" fn(TiFlashServerPtr, WriteCmdsView, RaftCmdHeader) -> u32,
    handle_admin_raft_cmd:
        extern "C" fn(TiFlashServerPtr, BaseBuffView, BaseBuffView, RaftCmdHeader) -> u32,
    handle_set_proxy: extern "C" fn(TiFlashServerPtr, *const TiFlashRaftProxyHelper),
    handle_destroy: extern "C" fn(TiFlashServerPtr, RegionId),
    handle_ingest_sst: extern "C" fn(TiFlashServerPtr, SnapshotViewArray, RaftCmdHeader) -> u32,
    handle_check_terminated: extern "C" fn(TiFlashServerPtr) -> u8,
    handle_compute_fs_stats: extern "C" fn(TiFlashServerPtr) -> FsStats,
    handle_get_tiflash_status: extern "C" fn(TiFlashServerPtr) -> u8,
    pre_handle_snapshot: extern "C" fn(
        TiFlashServerPtr,
        BaseBuffView,
        u64,
        SnapshotViewArray,
        u64,
        u64,
    ) -> RawCppPtr,
    apply_pre_handled_snapshot: extern "C" fn(TiFlashServerPtr, *const u8, u32),
    handle_get_table_sync_status: extern "C" fn(TiFlashServerPtr, u64) -> CppStrWithView,
    gc_raw_cpp_ptr: extern "C" fn(TiFlashServerPtr, RawCppPtr),

    is_tiflash_snapshot: extern "C" fn(TiFlashServerPtr, BaseBuffView) -> u8,
    gen_tiflash_snapshot: extern "C" fn(TiFlashServerPtr, RaftCmdHeader) -> RawCppPtr,
    serialize_tiflash_snapshot_into:
        extern "C" fn(TiFlashServerPtr, *const u8, BaseBuffView) -> SerializeTiFlashSnapshotRes,
    pre_handle_tiflash_snapshot:
        extern "C" fn(TiFlashServerPtr, BaseBuffView, u64, u64, u64, BaseBuffView) -> RawCppPtr,
    get_region_approximate_size_keys: extern "C" fn(
        TiFlashServerPtr,
        u64,
        BaseBuffView,
        BaseBuffView,
    ) -> GetRegionApproximateSizeKeysRes,
    scan_split_keys: extern "C" fn(
        TiFlashServerPtr,
        u64,
        BaseBuffView,
        BaseBuffView,
        CheckerConfig,
    ) -> ScanSplitKeysRes,
}

unsafe impl Send for TiFlashServerHelper {}

pub static mut TIFLASH_SERVER_HELPER_PTR: u64 = 0;

pub fn get_tiflash_server_helper() -> &'static TiFlashServerHelper {
    return unsafe { &(*(TIFLASH_SERVER_HELPER_PTR as *const TiFlashServerHelper)) };
}

pub fn get_tiflash_server_helper_mut() -> &'static mut TiFlashServerHelper {
    return unsafe { &mut (*(TIFLASH_SERVER_HELPER_PTR as *mut TiFlashServerHelper)) };
}

#[derive(Eq, PartialEq)]
pub enum TiFlashStatus {
    IDLE,
    Running,
    Stopped,
}

impl From<u8> for TiFlashStatus {
    fn from(s: u8) -> Self {
        match s {
            0 => TiFlashStatus::IDLE,
            1 => TiFlashStatus::Running,
            2 => TiFlashStatus::Stopped,
            _ => unreachable!(),
        }
    }
}

#[repr(C)]
pub struct CheckerConfig {
    pub max_size: u64,
    pub split_size: u64,
    pub batch_split_limit: u64,
}

impl TiFlashServerHelper {
    pub fn scan_split_keys(
        &self,
        region_id: u64,
        start_key: &[u8],
        end_key: &[u8],
        config: CheckerConfig,
    ) -> crate::errors::Result<(u64, u64, Vec<Vec<u8>>)> {
        let res = (self.scan_split_keys)(
            self.inner,
            region_id,
            start_key.into(),
            end_key.into(),
            config,
        );
        return if res.ok == 0 {
            Err(crate::errors::Error::Other(box_err!(
                "fail to scan split keys about region {} from tiflash",
                region_id
            )))
        } else {
            Ok((res.size, res.keys, res.split_keys.into()))
        };
    }

    pub fn get_region_approximate_size_keys_of_tiflash(
        &self,
        region: &metapb::Region,
    ) -> crate::errors::Result<(u64, u64)> {
        let start_key = region.get_start_key();
        let end_key = region.get_end_key();
        let res = (self.get_region_approximate_size_keys)(
            self.inner,
            region.get_id(),
            start_key.into(),
            end_key.into(),
        );
        return if res.ok == 0 {
            Err(crate::errors::Error::Other(box_err!(
                "fail to get region approximate size and keys about region {} from tiflash",
                region.get_id()
            )))
        } else {
            Ok((res.size, res.keys))
        };
    }

    pub fn is_tiflash_snapshot(&self, path: &[u8]) -> bool {
        (self.is_tiflash_snapshot)(self.inner, path.into()) != 0
    }

    pub fn gen_tiflash_snapshot(&self, header: RaftCmdHeader) -> RawCppPtr {
        (self.gen_tiflash_snapshot)(self.inner, header)
    }

    pub fn serialize_tiflash_snapshot_into(
        &self,
        p: *const u8,
        path: &[u8],
    ) -> SerializeTiFlashSnapshotRes {
        (self.serialize_tiflash_snapshot_into)(self.inner, p, path.into())
    }

    pub fn pre_handle_tiflash_snapshot(
        &self,
        region: &metapb::Region,
        peer_id: u64,
        index: u64,
        term: u64,
        path: BaseBuffView,
    ) -> RawCppPtr {
        (self.pre_handle_tiflash_snapshot)(
            self.inner,
            ProtoMsgBaseBuff::new(region).buff_view,
            peer_id,
            index,
            term,
            path,
        )
    }

    fn gc_raw_cpp_ptr(&self, p: RawCppPtr) {
        (self.gc_raw_cpp_ptr)(self.inner, p);
    }

    pub fn handle_get_table_sync_status(&self, table_id: u64) -> CppStrWithView {
        (self.handle_get_table_sync_status)(self.inner, table_id)
    }

    pub fn handle_compute_fs_stats(&self) -> FsStats {
        (self.handle_compute_fs_stats)(self.inner)
    }

    pub fn handle_write_raft_cmd(
        &self,
        cmds: &WriteCmds,
        header: RaftCmdHeader,
    ) -> TiFlashApplyRes {
        let res = (self.handle_write_raft_cmd)(self.inner, cmds.gen_view(), header);
        res.into()
    }

    pub fn handle_get_tiflash_status(&mut self) -> TiFlashStatus {
        (self.handle_get_tiflash_status)(self.inner).into()
    }

    pub fn handle_set_proxy(&mut self, proxy: *const TiFlashRaftProxyHelper) {
        (self.handle_set_proxy)(self.inner, proxy);
    }

    pub fn check(&self) {
        assert_eq!(std::mem::align_of::<Self>(), std::mem::align_of::<u64>());
        const MAGIC_NUMBER: u32 = 0x13579BDF;
        const VERSION: u32 = 12;

        if self.magic_number != MAGIC_NUMBER {
            eprintln!(
                "TiFlash Proxy FFI magic number not match: expect {} got {}",
                MAGIC_NUMBER, self.magic_number
            );
            std::process::exit(-1);
        } else if self.version != VERSION {
            eprintln!(
                "TiFlash Proxy FFI version not match: expect {} got {}",
                VERSION, self.version
            );
            std::process::exit(-1);
        }
    }

    pub fn handle_admin_raft_cmd(
        &self,
        req: &raft_cmdpb::AdminRequest,
        resp: &raft_cmdpb::AdminResponse,
        header: RaftCmdHeader,
    ) -> TiFlashApplyRes {
        let res = (self.handle_admin_raft_cmd)(
            self.inner,
            ProtoMsgBaseBuff::new(req).buff_view,
            ProtoMsgBaseBuff::new(resp).buff_view,
            header,
        );
        res.into()
    }

    pub fn pre_handle_snapshot(
        &self,
        region: &metapb::Region,
        peer_id: u64,
        snaps: &mut SnapshotHelper,
        index: u64,
        term: u64,
    ) -> RawCppPtr {
        (self.pre_handle_snapshot)(
            self.inner,
            ProtoMsgBaseBuff::new(region).buff_view,
            peer_id,
            snaps.gen_snapshot_view(),
            index,
            term,
        )
    }

    pub fn apply_pre_handled_snapshot(&self, s: *const u8, tp: u32) {
        (self.apply_pre_handled_snapshot)(self.inner, s, tp)
    }

    pub fn handle_ingest_sst(
        &self,
        snaps: &mut SnapshotHelper,
        header: RaftCmdHeader,
    ) -> TiFlashApplyRes {
        let res = (self.handle_ingest_sst)(self.inner, snaps.gen_snapshot_view(), header);
        res.into()
    }

    pub fn handle_destroy(&self, region_id: RegionId) {
        (self.handle_destroy)(self.inner, region_id);
    }

    pub fn handle_check_terminated(&self) -> bool {
        (self.handle_check_terminated)(self.inner) != 0
    }

    fn gen_cpp_string(&self, buff: &[u8]) -> *const u8 {
        (self.gen_cpp_string)(BaseBuffView {
            data: buff.as_ptr(),
            len: buff.len() as u64,
        })
        .into_raw()
    }
}
