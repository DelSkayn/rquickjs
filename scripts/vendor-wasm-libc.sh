#!/bin/sh
# Updates sys/vendor/wasi-libc/{include,lib} from the pinned wasi-sdk release.
# Re-running against an unchanged pin is a no-op, so CI runs this and diffs
# the result to catch drift. See sys/vendor/wasi-libc/NOTICE.md for details.

set -eu

WASI_SDK_VERSION_MAJOR=24
WASI_SDK_VERSION_MINOR=0
# sha256 of wasi-sysroot-24.0.tar.gz from the wasi-sdk-24 release.
WASI_SYSROOT_SHA256=35172f7d2799485b15a46b1d87f50a585d915ec662080f005d99153a50888f08

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
VENDOR_DIR="$REPO_ROOT/sys/vendor/wasi-libc"

# Headers quickjs's C sources need to compile against libc.a for this
# target; found via `-MD -MF` dependency tracking over libregexp.c,
# libunicode.c, quickjs.c, dtoa.c and wasm-shim/shim.c.
HEADERS="
__fd_set.h
__functions_malloc.h
__functions_memcpy.h
__header_inttypes.h
__header_stdlib.h
__header_string.h
__header_time.h
__header_unistd.h
__macro_FD_SETSIZE.h
__macro_PAGESIZE.h
__seek.h
__struct_iovec.h
__struct_timespec.h
__struct_timeval.h
__struct_tm.h
__typedef_clock_t.h
__typedef_clockid_t.h
__typedef_fd_set.h
__typedef_sigset_t.h
__typedef_suseconds_t.h
__typedef_time_t.h
alloca.h
assert.h
bits/alltypes.h
bits/limits.h
bits/posix.h
bits/stdint.h
ctype.h
features.h
inttypes.h
limits.h
math.h
stdbool.h
stdint.h
stdio.h
stdlib.h
string.h
strings.h
sys/select.h
sys/time.h
time.h
unistd.h
"

NOTICE_LINE1="/* Vendored from wasi-libc by scripts/vendor-wasm-libc.sh - do not edit."
NOTICE_LINE2="   MIT / Apache-2.0 WITH LLVM-exception; see sys/vendor/wasi-libc/NOTICE.md. */"

work_dir=$(mktemp -d)
trap 'rm -rf "$work_dir"' EXIT

archive="$work_dir/wasi-sysroot-$WASI_SDK_VERSION_MAJOR.$WASI_SDK_VERSION_MINOR.tar.gz"
uri="https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-$WASI_SDK_VERSION_MAJOR/wasi-sysroot-$WASI_SDK_VERSION_MAJOR.$WASI_SDK_VERSION_MINOR.tar.gz"

echo "Downloading $uri"
curl --location --fail --retry 5 -o "$archive" "$uri"

if command -v sha256sum >/dev/null 2>&1; then
    actual_sha256=$(sha256sum "$archive" | cut -d' ' -f1)
else
    actual_sha256=$(shasum -a 256 "$archive" | cut -d' ' -f1)
fi
if [ "$actual_sha256" != "$WASI_SYSROOT_SHA256" ]; then
    echo "error: sha256 mismatch for $archive: expected $WASI_SYSROOT_SHA256, got $actual_sha256" >&2
    exit 1
fi

tar -xzf "$archive" -C "$work_dir"
sysroot="$work_dir/wasi-sysroot-$WASI_SDK_VERSION_MAJOR.$WASI_SDK_VERSION_MINOR"

rm -rf "$VENDOR_DIR/include" "$VENDOR_DIR/lib"
mkdir -p "$VENDOR_DIR/include" "$VENDOR_DIR/lib"

for h in $HEADERS; do
    mkdir -p "$VENDOR_DIR/include/$(dirname "$h")"
    {
        echo "$NOTICE_LINE1"
        echo "$NOTICE_LINE2"
        cat "$sysroot/include/wasm32-wasi/$h"
    } >"$VENDOR_DIR/include/$h"
done

# wasi/api.h guards itself with `#ifndef __wasi__` / `#error`; comment out
# just that one line so it can be included outside an actual wasi target.
# (Defining __wasi__ instead would pin quickjs's stack_limit to 0.)
mkdir -p "$VENDOR_DIR/include/wasi"
{
    echo "$NOTICE_LINE1"
    echo "$NOTICE_LINE2"
    awk '
        /^#ifndef __wasi__/ { in_guard = 1 }
        /^#endif/ { in_guard = 0 }
        in_guard && /^#error/ { print "/* #error patched out by vendor-wasm-libc.sh for wasm32-unknown-unknown */"; next }
        { print }
    ' "$sysroot/include/wasm32-wasi/wasi/api.h"
} >"$VENDOR_DIR/include/wasi/api.h"

cp "$sysroot/lib/wasm32-wasi/libc.a" "$VENDOR_DIR/lib/libc.a"

echo "Vendored into $VENDOR_DIR"
