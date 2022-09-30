#![feature(box_patterns)]
#![feature(test)]
#![feature(custom_test_frameworks)]
#![test_runner(test_util::run_failpoint_tests)]
#![recursion_limit = "100"]

#[macro_use]
extern crate slog_global;

mod normal;
mod proxy;
mod util;
mod server_cluster_test;
