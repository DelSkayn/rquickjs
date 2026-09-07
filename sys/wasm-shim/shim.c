// OS-tail shims for wasm32-unknown-unknown: the only ambient authority is a
// single host-provided clock import. Everything else is a no-op or pure.
//
// This file is compiled into libquickjs.a for the wasm32-unknown-unknown
// target only. Because it lands in the same archive as quickjs.o, lld resolves
// it to a fixpoint before ever reaching wasi-libc, so each definition here
// keeps the corresponding wasi-libc member (and its wasi imports) out of the
// link entirely.
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <time.h>

// the single host import: microseconds since the unix epoch.
__attribute__((import_module("env"), import_name("__rquickjs_host_now_us")))
extern double __rquickjs_host_now_us(void);

int gettimeofday(struct timeval *restrict tv, void *restrict tz) {
    (void)tz;
    long long us = (long long)__rquickjs_host_now_us();
    tv->tv_sec = (time_t)(us / 1000000);
    tv->tv_usec = (suseconds_t)(us % 1000000);
    return 0;
}

int clock_gettime(clockid_t clock, struct timespec *ts) {
    (void)clock;
    long long us = (long long)__rquickjs_host_now_us();
    ts->tv_sec = (time_t)(us / 1000000);
    ts->tv_nsec = (long)((us % 1000000) * 1000);
    return 0;
}

// sandboxed scripts run in UTC: no tz database, offset 0. quickjs only reads
// tm_gmtoff on this path.
struct tm *localtime_r(const time_t *restrict t, struct tm *restrict tm) {
    (void)t;
    memset(tm, 0, sizeof(*tm));
    return tm;
}

_Noreturn void abort(void) { __builtin_trap(); }

_Noreturn void __assert_fail(const char *expr, const char *file, int line,
                             const char *func) {
    (void)expr;
    (void)file;
    (void)line;
    (void)func;
    __builtin_trap();
}

// stdout/stderr do not exist here; the dump/debug paths become no-ops and the
// musl FILE machinery (and its fd_write imports) is never pulled in.
int fprintf(FILE *restrict f, const char *restrict fmt, ...) {
    (void)f;
    (void)fmt;
    return 0;
}
int fputc(int c, FILE *f) {
    (void)f;
    return c;
}
int putchar(int c) { return c; }
int fputs(const char *restrict s, FILE *restrict f) {
    (void)s;
    (void)f;
    return 0;
}
// no fwrite shim: musl's fwrite.o also defines __fwritex, which the snprintf
// core pulls in, so shimming fwrite would collide.
int fflush(FILE *f) {
    (void)f;
    return 0;
}

// The stdio backends. `printf` pulls in stdout.o for `__stdout_FILE`, whose
// FILE initialiser names these three; `__stdout_write` in turn names
// `__stdio_write` and `__isatty`. Left to wasi-libc those five members are the
// sole source of the `fd_write` / `fd_seek` / `fd_close` / `fd_fdstat_get`
// imports, so defining them here removes the last wasi import from the module.
// Signatures must match musl's `stdio_impl.h` exactly or wasm-ld rejects the
// indirect calls through the FILE vtable.
size_t __stdio_write(FILE *f, const unsigned char *buf, size_t len) {
    (void)f;
    (void)buf;
    return len;
}

size_t __stdout_write(FILE *f, const unsigned char *buf, size_t len) {
    (void)f;
    (void)buf;
    return len;
}

off_t __stdio_seek(FILE *f, off_t off, int whence) {
    (void)f;
    (void)off;
    (void)whence;
    return -1;
}

int __stdio_close(FILE *f) {
    (void)f;
    return 0;
}

int __isatty(int fd) {
    (void)fd;
    return 0;
}

// no `__towrite` shim either: `__fwritex` calls it to arm the write buffer of
// the stack FILE that `snprintf` builds, so stubbing it corrupts every
// number-to-string conversion quickjs performs.
