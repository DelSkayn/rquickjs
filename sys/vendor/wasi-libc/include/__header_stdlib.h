/* Vendored from wasi-libc by scripts/vendor-wasm-libc.sh - do not edit.
   MIT / Apache-2.0 WITH LLVM-exception; see sys/vendor/wasi-libc/NOTICE.md. */
#ifndef __wasilibc___header_stdlib_h
#define __wasilibc___header_stdlib_h

#define __need_size_t
#include <stddef.h>

#include <__functions_malloc.h>

#ifdef __cplusplus
extern "C" {
#endif

void abort(void) __attribute__((__noreturn__));
void qsort(void *, size_t, size_t, int (*)(const void *, const void *));
void _Exit(int) __attribute__((__noreturn__));

#ifdef __cplusplus
}
#endif

#endif
