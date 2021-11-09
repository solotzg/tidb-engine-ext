if [ ${CLEAN:-0} -ne 0 ]; then
  cargo clean
fi

TEST_THREAD=

if [ ${GENERATE_COV:-0} -ne 0 ]; then
  export RUST_BACKTRACE=1
  export RUSTFLAGS="-Zinstrument-coverage"
  export LLVM_PROFILE_FILE="tidb-engine-ext-%p-%m.profraw"
  rustup component list | grep "llvm-tools-preview-x86_64-unknown-linux-gnu (installed)"
  if [ $? -ne 0 ]; then
    rustup component add llvm-tools-preview
  fi
  cargo install --list | grep grcov
  if [ $? -ne 0 ]; then
    cargo install grcov
  fi
  export TEST_THREAD="--test-threads 1"
  find . -name "*.profraw" -type f -delete
fi

cargo test --package tests --test failpoints -- cases::test_normal $TEST_THREAD && \
cargo test --package tests --test failpoints -- cases::test_bootstrap $TEST_THREAD && \
cargo test --package tests --test failpoints -- cases::test_compact_log $TEST_THREAD && \
cargo test --package tests --test failpoints -- cases::test_early_apply $TEST_THREAD && \
cargo test --package tests --test failpoints -- cases::test_encryption $TEST_THREAD && \
cargo test --package tests --test failpoints -- cases::test_pd_client $TEST_THREAD && \
cargo test --package tests --test failpoints -- cases::test_pending_peers $TEST_THREAD && \
cargo test --package tests --test failpoints -- cases::test_transaction $TEST_THREAD && \
cargo test --package tests --test failpoints -- cases::test_cmd_epoch_checker $TEST_THREAD && \
cargo test --package tests --test failpoints -- cases::test_disk_full $TEST_THREAD && \
cargo test --package tests --test failpoints -- cases::test_stale_peer $TEST_THREAD && \
cargo test --package tests --test failpoints -- cases::test_import_service $TEST_THREAD && \
cargo test --package tests --test failpoints -- cases::test_split_region --skip test_report_approximate_size_after_split_check $TEST_THREAD && \
cargo test --package tests --test failpoints -- cases::test_snap $TEST_THREAD && \
cargo test --package tests --test failpoints -- cases::test_merge $TEST_THREAD && \
cargo test --package tests --test failpoints -- cases::test_replica_read $TEST_THREAD && \
# TiFlash do not support stale read currently
#cargo test --package tests --test failpoints -- cases::test_replica_stale_read $TEST_THREAD && \
cargo test --package tests --test failpoints -- cases::test_server $TEST_THREAD

cargo test --package tests --test integrations -- raftstore::test_bootstrap $TEST_THREAD && \
cargo test --package tests --test integrations -- raftstore::test_clear_stale_data $TEST_THREAD && \
cargo test --package tests --test integrations -- raftstore::test_compact_after_delete $TEST_THREAD && \
cargo test --package tests --test integrations -- raftstore::test_compact_log $TEST_THREAD && \
## Sometimes fails
#cargo test --package tests --test integrations -- raftstore::test_conf_change $TEST_THREAD && \
cargo test --package tests --test integrations -- raftstore::test_early_apply $TEST_THREAD && \
cargo test --package tests --test integrations -- raftstore::test_hibernate $TEST_THREAD && \
cargo test --package tests --test integrations -- raftstore::test_joint_consensus $TEST_THREAD && \
cargo test --package tests --test integrations -- raftstore::test_replica_read $TEST_THREAD && \
cargo test --package tests --test integrations -- raftstore::test_snap $TEST_THREAD && \
# Sometimes fails
#cargo test --package tests --test integrations -- raftstore::test_split_region $TEST_THREAD && \
cargo test --package tests --test integrations -- raftstore::test_stale_peer $TEST_THREAD && \
cargo test --package tests --test integrations -- raftstore::test_status_command $TEST_THREAD && \
cargo test --package tests --test integrations -- raftstore::test_prevote $TEST_THREAD && \
cargo test --package tests --test integrations -- raftstore::test_region_change_observer $TEST_THREAD && \
cargo test --package tests --test integrations -- raftstore::test_region_heartbeat $TEST_THREAD && \
cargo test --package tests --test integrations -- raftstore::test_region_info_accessor $TEST_THREAD && \
cargo test --package tests --test integrations -- raftstore::test_transfer_leader $TEST_THREAD && \
cargo test --package tests --test integrations -- raftstore::test_single $TEST_THREAD && \
# Sometimes fails
cargo test --package tests --test integrations -- raftstore::test_merge $TEST_THREAD && \
cargo test --package tests --test integrations -- raftstore::test_tombstone $TEST_THREAD && \
cargo test --package tests --test integrations -- server::kv_service::test_read_index_check_memory_locks $TEST_THREAD && \
cargo test --package tests --test integrations -- raftstore::test_batch_read_index $TEST_THREAD && \
cargo test --package tests --test integrations -- import::test_upload_sst $TEST_THREAD && \


if [ ${GENERATE_COV:-0} -ne 0 ]; then
  grcov . --binary-path target/debug/ . -t html --branch --ignore-not-existing -o ./coverage/
fi