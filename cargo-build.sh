#!/bin/bash

cargo_build_extra_parameter=""
if [[ ! -z ${PROXY_BUILD_TYPE} ]]; then
  cargo_build_extra_parameter="--${PROXY_BUILD_TYPE}"
fi

set -ex

export CFLAGS=-w
export CXXFLAGS=-w

cargo build --no-default-features --features "${PROXY_ENABLE_FEATURES}" ${cargo_build_extra_parameter}
