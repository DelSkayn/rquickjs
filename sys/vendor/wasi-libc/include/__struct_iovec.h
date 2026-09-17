/* Vendored from wasi-libc by scripts/vendor-wasm-libc.sh - do not edit.
   MIT / Apache-2.0 WITH LLVM-exception; see sys/vendor/wasi-libc/NOTICE.md. */
#ifndef __wasilibc___struct_iovec_h
#define __wasilibc___struct_iovec_h

#define __need_size_t
#include <stddef.h>

struct iovec {
    void *iov_base;
    size_t iov_len;
};

#endif
