/* Vendored from wasi-libc by scripts/vendor-wasm-libc.sh - do not edit.
   MIT / Apache-2.0 WITH LLVM-exception; see sys/vendor/wasi-libc/NOTICE.md. */
#ifndef	_ALLOCA_H
#define	_ALLOCA_H

#ifdef __cplusplus
extern "C" {
#endif

#define	__NEED_size_t
#include <bits/alltypes.h>

void *alloca(size_t);

#define alloca __builtin_alloca

#ifdef __cplusplus
}
#endif

#endif
