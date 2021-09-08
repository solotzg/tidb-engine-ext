#pragma once

#ifndef RAFT_STORE_PROXY_FII_TYPES
#define RAFT_STORE_PROXY_FII_TYPES

#define PROXY_FII_TYPE_INT8 signed char
#define PROXY_FII_TYPE_UINT8 unsigned char
#define PROXY_FII_TYPE_INT16 signed short int
#define PROXY_FII_TYPE_UINT16 unsigned short int
#define PROXY_FII_TYPE_INT32 signed int
#define PROXY_FII_TYPE_UINT32 unsigned int
#define PROXY_FII_TYPE_INT64 signed long int
#define PROXY_FII_TYPE_UINT64 unsigned long int

#endif

namespace DB {
using ConstRawVoidPtr = const void *;
using RawVoidPtr = void *;
}  // namespace DB
