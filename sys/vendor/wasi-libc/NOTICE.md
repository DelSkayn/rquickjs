# Vendored wasi-libc

`include/` and `lib/libc.a` in this directory are vendored from the
wasi-sdk-24 release (`wasi-sysroot-24.0.tar.gz`,
sha256 `35172f7d2799485b15a46b1d87f50a585d915ec662080f005d99153a50888f08`)
by `scripts/vendor-wasm-libc.sh`. Do not hand-edit them; re-run the script.

- `lib/libc.a`: the wasm32-wasi build of wasi-libc, unmodified.
- `include/`: the subset of wasi-libc's public headers that quickjs's C
  sources need to compile against `libc.a` for `wasm32-unknown-unknown`.
  `wasi/api.h` carries one commented-out `#error` (see the patch step in
  `vendor-wasm-libc.sh`) so it can be included outside an actual wasi
  target; the rest are otherwise unmodified aside from the provenance
  comment the script prepends to each file.

wasi-libc is licensed MIT / Apache-2.0 WITH LLVM-exception / Apache-2.0
(https://github.com/WebAssembly/wasi-libc/blob/main/LICENSE); wasi-sdk,
which builds and publishes this compiled artifact, is Apache-2.0 WITH
LLVM-exception (https://github.com/WebAssembly/wasi-sdk/blob/main/LICENSE).
Both are permissive with no copyleft terms; only `libc.a`'s object code and
the headers above are vendored here, not wasi-sdk's clang/LLVM toolchain.

To update: bump `WASI_SDK_VERSION_MAJOR`/`MINOR` and `WASI_SYSROOT_SHA256`
at the top of `vendor-wasm-libc.sh` (the checksum is published on the
release page, https://github.com/WebAssembly/wasi-sdk/releases/tag/wasi-sdk-<major>),
then re-run `sh scripts/vendor-wasm-libc.sh` from the repo root.
