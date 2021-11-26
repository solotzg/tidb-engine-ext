#pragma once

#include "Common.h"

namespace DB {
enum class ColumnFamilyType : uint8_t {
  Lock = 0,
  Write,
  Default,
  asd,
};
}  // namespace DB
