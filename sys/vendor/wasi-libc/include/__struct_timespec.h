/* Vendored from wasi-libc by scripts/vendor-wasm-libc.sh - do not edit.
   MIT / Apache-2.0 WITH LLVM-exception; see sys/vendor/wasi-libc/NOTICE.md. */
#ifndef __wasilibc___struct_timespec_h
#define __wasilibc___struct_timespec_h

#include <__typedef_time_t.h>

/* As specified in POSIX. */
struct timespec {
    time_t tv_sec;
    long tv_nsec;
};

#endif
