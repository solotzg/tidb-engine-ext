#pragma once

#include "Common.h"

namespace DB {
enum class ColumnFamilyType : PROXY_FII_TYPE_UINT8 {
  Lock = 0,
  Write,
  Default,
};
}  // namespace DB
