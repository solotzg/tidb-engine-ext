
fn main() {
    println!(
    "cargo:rerun-if-changed={}",
    "components/raftstore/src/engine_store_ffi/interfaces.rs"
    );
    std::process::Command::new("touch").args(&["./aaa.txt"]).output().expect("sh exec error!");
    gen_proxy_ffi::gen_ffi_code();
}
