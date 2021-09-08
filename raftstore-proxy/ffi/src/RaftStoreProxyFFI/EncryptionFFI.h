#pragma once

#include "Common.h"

namespace DB {
struct RawCppString;
using RawCppStringPtr = RawCppString *;
enum class FileEncryptionRes : PROXY_FII_TYPE_UINT8 {
  Disabled = 0,
  Ok,
  Error,
};

enum class EncryptionMethod : PROXY_FII_TYPE_UINT8 {
  Unknown = 0,
  Plaintext,
  Aes128Ctr,
  Aes192Ctr,
  Aes256Ctr,
};
struct FileEncryptionInfoRaw {
  FileEncryptionRes res;
  EncryptionMethod method;
  RawCppStringPtr key;
  RawCppStringPtr iv;
  RawCppStringPtr error_msg;
};
}  // namespace DB
