#!/bin/bash

set -e

source env.sh

export PROXY_BUILD_TYPE=release
export PROXY_PROFILE=release
if [[ $(uname -s) == "Darwin" ]]; then
  echo "Kernel is Darwin, change build type to debug"
  unset PROXY_BUILD_TYPE
  export PROXY_PROFILE=debug

  export OPENSSL_ROOT_DIR=$(brew --prefix openssl@1.1)
  export OPENSSL_NO_VENDOR=1
  export OPENSSL_STATIC=1

  mkdir -p target/release
  PROXY_LIB_TARGET_COPY_PATH="target/release/lib${ENGINE_LABEL_VALUE}_proxy.dylib" make build
else
  make build
fi
