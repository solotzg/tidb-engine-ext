fn main() {
    println!(
        "cargo:rerun-if-changed={}",
        "components/raftstore/src/engine_store_ffi/interfaces.rs"
    );
    println!(
        "cargo:rerun-if-changed={}",
        "raftstore-proxy/ffi/src/RaftStoreProxyFFI"
    );
    gen_proxy_ffi::gen_ffi_code();
}
