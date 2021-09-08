#[allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]
pub mod root {
    #[allow(unused_imports)]
    use self::super::root;
    pub mod DB {
        #[allow(unused_imports)]
        use self::super::super::root;
        pub type ConstRawVoidPtr = *const ::std::os::raw::c_void;
        pub type RawVoidPtr = *mut ::std::os::raw::c_void;
        #[repr(u8)]
        #[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
        pub enum ColumnFamilyType {
            Lock = 0,
            Write = 1,
            Default = 2,
        }
        #[repr(C)]
        #[derive(Debug)]
        pub struct RawCppString {
            _unused: [u8; 0],
        }
        pub type RawCppStringPtr = *mut root::DB::RawCppString;
        #[repr(u8)]
        #[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
        pub enum FileEncryptionRes {
            Disabled = 0,
            Ok = 1,
            Error = 2,
        }
        #[repr(u8)]
        #[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
        pub enum EncryptionMethod {
            Unknown = 0,
            Plaintext = 1,
            Aes128Ctr = 2,
            Aes192Ctr = 3,
            Aes256Ctr = 4,
        }
        #[repr(C)]
        #[derive(Debug)]
        pub struct FileEncryptionInfoRaw {
            pub res: root::DB::FileEncryptionRes,
            pub method: root::DB::EncryptionMethod,
            pub key: root::DB::RawCppStringPtr,
            pub iv: root::DB::RawCppStringPtr,
            pub error_msg: root::DB::RawCppStringPtr,
        }
        #[repr(C)]
        #[derive(Debug)]
        pub struct EngineStoreServerWrap {
            _unused: [u8; 0],
        }
        #[repr(u32)]
        #[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
        pub enum EngineStoreApplyRes {
            None = 0,
            Persist = 1,
            NotFound = 2,
        }
        #[repr(u8)]
        #[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
        pub enum WriteCmdType {
            Put = 0,
            Del = 1,
        }
        #[repr(C)]
        #[derive(Debug)]
        pub struct BaseBuffView {
            pub data: *const ::std::os::raw::c_char,
            pub len: ::std::os::raw::c_ulong,
        }
        #[repr(C)]
        #[derive(Debug)]
        pub struct RaftCmdHeader {
            pub region_id: ::std::os::raw::c_ulong,
            pub index: ::std::os::raw::c_ulong,
            pub term: ::std::os::raw::c_ulong,
        }
        #[repr(C)]
        #[derive(Debug)]
        pub struct WriteCmdsView {
            pub keys: *const root::DB::BaseBuffView,
            pub vals: *const root::DB::BaseBuffView,
            pub cmd_types: *const root::DB::WriteCmdType,
            pub cmd_cf: *const root::DB::ColumnFamilyType,
            pub len: ::std::os::raw::c_ulong,
        }
        #[repr(C)]
        #[derive(Debug)]
        pub struct FsStats {
            pub used_size: ::std::os::raw::c_ulong,
            pub avail_size: ::std::os::raw::c_ulong,
            pub capacity_size: ::std::os::raw::c_ulong,
            pub ok: ::std::os::raw::c_uchar,
        }
        #[repr(C)]
        #[derive(Debug)]
        pub struct StoreStats {
            pub fs_stats: root::DB::FsStats,
            pub engine_bytes_written: ::std::os::raw::c_ulong,
            pub engine_keys_written: ::std::os::raw::c_ulong,
            pub engine_bytes_read: ::std::os::raw::c_ulong,
            pub engine_keys_read: ::std::os::raw::c_ulong,
        }
        #[repr(u8)]
        #[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
        pub enum RaftProxyStatus {
            Idle = 0,
            Running = 1,
            Stopped = 2,
        }
        #[repr(u8)]
        #[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
        pub enum EngineStoreServerStatus {
            Idle = 0,
            Running = 1,
            Stopped = 2,
        }
        pub type RawCppPtrType = ::std::os::raw::c_uint;
        #[repr(C)]
        #[derive(Debug)]
        pub struct RawCppPtr {
            pub ptr: root::DB::RawVoidPtr,
            pub type_: root::DB::RawCppPtrType,
        }
        #[repr(C)]
        #[derive(Debug)]
        pub struct CppStrWithView {
            pub inner: root::DB::RawCppPtr,
            pub view: root::DB::BaseBuffView,
        }
        #[repr(u8)]
        #[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
        pub enum HttpRequestStatus {
            Ok = 0,
            ErrorParam = 1,
        }
        #[repr(C)]
        #[derive(Debug)]
        pub struct HttpRequestRes {
            pub status: root::DB::HttpRequestStatus,
            pub res: root::DB::CppStrWithView,
        }
        #[repr(C)]
        #[derive(Debug)]
        pub struct CppStrVecView {
            pub view: *const root::DB::BaseBuffView,
            pub len: ::std::os::raw::c_ulong,
        }
        #[repr(C)]
        #[derive(Debug)]
        pub struct SSTView {
            pub type_: root::DB::ColumnFamilyType,
            pub path: root::DB::BaseBuffView,
        }
        #[repr(C)]
        #[derive(Debug)]
        pub struct SSTViewVec {
            pub views: *const root::DB::SSTView,
            pub len: ::std::os::raw::c_ulong,
        }
        #[repr(C)]
        #[derive(Debug)]
        pub struct RaftStoreProxyPtr {
            pub inner: root::DB::ConstRawVoidPtr,
        }
        #[repr(C)]
        #[derive(Debug)]
        pub struct SSTReaderPtr {
            pub inner: root::DB::RawVoidPtr,
        }
        #[repr(C)]
        #[derive(Debug)]
        pub struct SSTReaderInterfaces {
            pub fn_get_sst_reader: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: root::DB::SSTView,
                    arg2: root::DB::RaftStoreProxyPtr,
                ) -> root::DB::SSTReaderPtr,
            >,
            pub fn_remained: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: root::DB::SSTReaderPtr,
                    arg2: root::DB::ColumnFamilyType,
                ) -> ::std::os::raw::c_uchar,
            >,
            pub fn_key: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: root::DB::SSTReaderPtr,
                    arg2: root::DB::ColumnFamilyType,
                ) -> root::DB::BaseBuffView,
            >,
            pub fn_value: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: root::DB::SSTReaderPtr,
                    arg2: root::DB::ColumnFamilyType,
                ) -> root::DB::BaseBuffView,
            >,
            pub fn_next: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: root::DB::SSTReaderPtr,
                    arg2: root::DB::ColumnFamilyType,
                ),
            >,
            pub fn_gc: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: root::DB::SSTReaderPtr,
                    arg2: root::DB::ColumnFamilyType,
                ),
            >,
        }
        #[repr(C)]
        #[derive(Debug)]
        pub struct RaftStoreProxyFFIHelper {
            pub proxy_ptr: root::DB::RaftStoreProxyPtr,
            pub fn_handle_get_proxy_status: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: root::DB::RaftStoreProxyPtr,
                ) -> root::DB::RaftProxyStatus,
            >,
            pub fn_is_encryption_enabled: ::std::option::Option<
                unsafe extern "C" fn(arg1: root::DB::RaftStoreProxyPtr) -> ::std::os::raw::c_uchar,
            >,
            pub fn_encryption_method: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: root::DB::RaftStoreProxyPtr,
                ) -> root::DB::EncryptionMethod,
            >,
            pub fn_handle_get_file: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: root::DB::RaftStoreProxyPtr,
                    arg2: root::DB::BaseBuffView,
                ) -> root::DB::FileEncryptionInfoRaw,
            >,
            pub fn_handle_new_file: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: root::DB::RaftStoreProxyPtr,
                    arg2: root::DB::BaseBuffView,
                ) -> root::DB::FileEncryptionInfoRaw,
            >,
            pub fn_handle_delete_file: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: root::DB::RaftStoreProxyPtr,
                    arg2: root::DB::BaseBuffView,
                ) -> root::DB::FileEncryptionInfoRaw,
            >,
            pub fn_handle_link_file: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: root::DB::RaftStoreProxyPtr,
                    arg2: root::DB::BaseBuffView,
                    arg3: root::DB::BaseBuffView,
                ) -> root::DB::FileEncryptionInfoRaw,
            >,
            pub fn_handle_batch_read_index: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: root::DB::RaftStoreProxyPtr,
                    arg2: root::DB::CppStrVecView,
                    arg3: root::DB::RawVoidPtr,
                    arg4: ::std::os::raw::c_ulong,
                ),
            >,
            pub sst_reader_interfaces: root::DB::SSTReaderInterfaces,
            pub fn_server_info: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: root::DB::RaftStoreProxyPtr,
                    arg2: root::DB::BaseBuffView,
                    arg3: root::DB::RawVoidPtr,
                ) -> ::std::os::raw::c_uint,
            >,
        }
        #[repr(C)]
        #[derive(Debug)]
        pub struct EngineStoreServerHelper {
            pub magic_number: ::std::os::raw::c_uint,
            pub version: ::std::os::raw::c_ulong,
            pub inner: *mut root::DB::EngineStoreServerWrap,
            pub fn_gen_cpp_string: ::std::option::Option<
                unsafe extern "C" fn(arg1: root::DB::BaseBuffView) -> root::DB::RawCppPtr,
            >,
            pub fn_handle_write_raft_cmd: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: *const root::DB::EngineStoreServerWrap,
                    arg2: root::DB::WriteCmdsView,
                    arg3: root::DB::RaftCmdHeader,
                ) -> root::DB::EngineStoreApplyRes,
            >,
            pub fn_handle_admin_raft_cmd: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: *const root::DB::EngineStoreServerWrap,
                    arg2: root::DB::BaseBuffView,
                    arg3: root::DB::BaseBuffView,
                    arg4: root::DB::RaftCmdHeader,
                ) -> root::DB::EngineStoreApplyRes,
            >,
            pub fn_atomic_update_proxy: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: *mut root::DB::EngineStoreServerWrap,
                    arg2: *mut root::DB::RaftStoreProxyFFIHelper,
                ),
            >,
            pub fn_handle_destroy: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: *mut root::DB::EngineStoreServerWrap,
                    arg2: ::std::os::raw::c_ulong,
                ),
            >,
            pub fn_handle_ingest_sst: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: *mut root::DB::EngineStoreServerWrap,
                    arg2: root::DB::SSTViewVec,
                    arg3: root::DB::RaftCmdHeader,
                ) -> root::DB::EngineStoreApplyRes,
            >,
            pub fn_handle_check_terminated: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: *mut root::DB::EngineStoreServerWrap,
                ) -> ::std::os::raw::c_uchar,
            >,
            pub fn_handle_compute_store_stats: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: *mut root::DB::EngineStoreServerWrap,
                ) -> root::DB::StoreStats,
            >,
            pub fn_handle_get_engine_store_server_status: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: *mut root::DB::EngineStoreServerWrap,
                ) -> root::DB::EngineStoreServerStatus,
            >,
            pub fn_pre_handle_snapshot: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: *mut root::DB::EngineStoreServerWrap,
                    arg2: root::DB::BaseBuffView,
                    arg3: ::std::os::raw::c_ulong,
                    arg4: root::DB::SSTViewVec,
                    arg5: ::std::os::raw::c_ulong,
                    arg6: ::std::os::raw::c_ulong,
                ) -> root::DB::RawCppPtr,
            >,
            pub fn_apply_pre_handled_snapshot: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: *mut root::DB::EngineStoreServerWrap,
                    arg2: root::DB::RawVoidPtr,
                    arg3: root::DB::RawCppPtrType,
                ),
            >,
            pub fn_handle_http_request: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: *mut root::DB::EngineStoreServerWrap,
                    arg2: root::DB::BaseBuffView,
                ) -> root::DB::HttpRequestRes,
            >,
            pub fn_check_http_uri_available: ::std::option::Option<
                unsafe extern "C" fn(arg1: root::DB::BaseBuffView) -> ::std::os::raw::c_uchar,
            >,
            pub fn_gc_raw_cpp_ptr: ::std::option::Option<
                unsafe extern "C" fn(arg1: root::DB::RawVoidPtr, arg2: root::DB::RawCppPtrType),
            >,
            pub fn_insert_batch_read_index_resp: ::std::option::Option<
                unsafe extern "C" fn(
                    arg1: root::DB::RawVoidPtr,
                    arg2: root::DB::BaseBuffView,
                    arg3: ::std::os::raw::c_ulong,
                ),
            >,
            pub fn_set_server_info_resp: ::std::option::Option<
                unsafe extern "C" fn(arg1: root::DB::BaseBuffView, arg2: root::DB::RawVoidPtr),
            >,
        }
        pub const RAFT_STORE_PROXY_VERSION: ::std::os::raw::c_ulong = 2145788594598211678;
        pub const RAFT_STORE_PROXY_MAGIC_NUMBER: ::std::os::raw::c_uint = 324508639;
    }
}
