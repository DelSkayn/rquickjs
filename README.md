# rquickjs

[![github](https://img.shields.io/badge/github-delskayn/rquickjs-8da0cb.svg?style=for-the-badge&logo=github)](https://github.com/DelSkayn/rquickjs)
[![crates](https://img.shields.io/crates/v/rquickjs.svg?style=for-the-badge&color=fc8d62&logo=rust)](https://crates.io/crates/rquickjs)
[![docs](https://img.shields.io/badge/docs.rs-rquickjs-66c2a5?style=for-the-badge&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K)](https://docs.rs/rquickjs)
[![status](https://img.shields.io/github/actions/workflow/status/DelSkayn/rquickjs/ci.yml?branch=master&style=for-the-badge&logo=github-actions&logoColor=white)](https://github.com/DelSkayn/rquickjs/actions/workflows/ci.yml)

This library is a high level bindings of the [QuickJS-NG](https://quickjs-ng.github.io/quickjs/) JavaScript engine, a fork of the [QuickJS](https://bellard.org/quickjs/) Javascript engine.
Its goal is to be an easy to use, and safe wrapper similar to the rlua library.

**QuickJS** is a small and embeddable JavaScript engine. It supports the _ES2020_ specification including modules, asynchronous generators, proxies and BigInt.
It optionally supports mathematical extensions such as big decimal floating point numbers (BigDecimal), big binary floating point numbers (BigFloat) and operator overloading.

## Main features of QuickJS

- Small and easily embeddable: just a few C files, no external dependency, 210 KiB of x86 code for a simple hello world program.
- Fast interpreter with very low startup time: runs the 75000 tests of the ECMAScript Test Suite in about 100 seconds on a single core of a desktop PC.
  The complete life cycle of a runtime instance completes in less than 300 microseconds.
- Almost complete ES2020 support including modules, asynchronous generators and full Annex B support (legacy web compatibility).
- Passes nearly 100% of the ECMAScript Test Suite tests when selecting the ES2020 features. A summary is available at Test262 Report.
- Can compile JavaScript sources to executables with no external dependency.
- Garbage collection using reference counting (to reduce memory usage and have deterministic behavior) with cycle removal.
- Mathematical extensions: BigDecimal, BigFloat, operator overloading, bigint mode, math mode.
- Command line interpreter with contextual colorization implemented in JavaScript.
- Small built-in standard library with C library wrappers.

## Features provided by this crate

- Full integration with async Rust
  - The ES6 Promises can be handled as Rust futures and vice versa
  - Easy integration with almost any async runtime or executor
- Flexible data conversion between Rust and JS
  - Many widely used Rust types can be converted to JS and vice versa
- Support for user-defined allocators
  - The `Runtime` can be created using custom allocator
  - Using Rust's global allocator is also fully supported
- Support for user-defined module resolvers and loaders which also
  can be combined to get more flexible solution for concrete case
- Support for bundling JS modules as a bytecode using `embed` macro
- Support for deferred calling of JS functions
- Full support of ES6 classes
  - Rust data types can be represented as JS classes
  - Data fields can be accessed via object properties
  - Both static and instance members is also supported
  - The properties can be defined with getters and setters
  - Support for constant static properties
  - Support for holding references to JS objects
    (Data type which holds refs should implement `Trace` trait to get garbage collector works properly)
  - Support for extending defined classes by JS

## Community development

This crate doesn't aim to provide system and web APIs. The QuickJS library is close to [V8](https://v8.dev/) in that regard.
If you need APIs from [WinterGC](https://wintercg.org/) or [Node](https://nodejs.org/api/), then you can take a look at the follow community projects:

- [AWS LLRT Modules](https://github.com/awslabs/llrt/tree/main/llrt_modules): Collection of modules that micmic some of the `Node` APIs in pure Rust
- [Rquickjs Extra](https://github.com/rquickjs/rquickjs-extra): Collection of modules that complement `AWS LLRT Modules` in pure Rust

The community has also built various utilities which might be relevant to you:

- [Rquickjs Serde](https://github.com/rquickjs/rquickjs-serde): Serde serializer and deserializer for rquickjs Value

## Development status

This bindings is feature complete, mostly stable and ready to use.
The error handling is only thing which may change in the future.
Some experimental features like `parallel` may not works as expected. Use it for your own risk.

## Supported platforms

Rquickjs needs to compile a C-library which has it's own limitation on supported platforms, furthermore it needs to generate bindings for that platform.
As a result rquickjs might not compile on all platforms which rust supports.
In general you can allways try to compile rquickjs with the `bindgen` feature, this should work for most platforms.
Rquickjs ships bindings for a limited set of platforms, for these platforms you don't have to enable the `bindgen` feature.
See below for a list of supported platforms.

| **platform**                   | **shipped bindings** | **tested** | **supported by quickjs** |
| ------------------------------ | :------------------: | :--------: | :----------------------: |
|                                |                      |            |                          |
| x86_64-unknown-linux-gnu       |          ✅          |     ✅     |            ✅            |
| i686-unknown-linux-gnu         |          ✅          |     ✅     |            ✅            |
| aarch64-unknown-linux-gnu      |          ✅          |     ✅     |            ✅            |
| loongarch64-unknown-linux-gnu  |          ✅          |     ✅     |            ✅            |
| x86_64-unknown-linux-musl      |          ✅          |     ✅     |            ✅            |
| aarch64-unknown-linux-musl     |          ✅          |     ✅     |            ✅            |
| loongarch64-unknown-linux-musl |          ✅          |     ✅     |            ✅            |
| x86_64-pc-windows-gnu          |          ✅          |     ✅     |            ✅            |
| i686-pc-windows-gnu            |          ✅          |     ✅     |            ✅            |
| x86_64-pc-windows-msvc         |          ✅          |     ✅     |     ❌ experimental!     |
| aarch64-pc-windows-msvc        |          ✅          |     ❌     |     ❌ experimental!     |
| x86_64-apple-darwin            |          ✅          |     ✅     |            ✅            |
| aarch64-apple-darwin           |          ✅          |     ❌     |            ✅            |
| wasm32-wasip1                  |          ✅          |     ✅     |            ✅            |
| wasm32-wasip2                  |          ✅          |     ✅     |            ✅            |
| wasm32-unknown-unknown         |          ✅          |     ✅     |            ✅            |
| other                          |          ❌          |     ❌     |         Unknown          |

## wasm32-unknown-unknown

`wasm32-unknown-unknown` is supported out of the box: add rquickjs as a dependency and build for the target. No feature flag, no extra crate, no wasi runtime.

The target ships no libc, so `rquickjs-sys` supplies the whole OS tail itself:

- quickjs is compiled with the same `EMSCRIPTEN` / `FE_DOWNWARD` / `FE_UPWARD` defines already used for wasi, against a wasi-sysroot include tree.
- the pure-compute members of wasi-libc (`snprintf`, `strtod`, libm, dlmalloc) are linked statically.
- `sys/wasm-shim/shim.c` replaces every member that would otherwise reach for the host: the clock, `localtime_r`, `abort`/`__assert_fail`, and the stdio backends.

The result imports **exactly one** host function and nothing else.

### The `__rquickjs_host_now_us` import

The embedder must supply a single import, `env.__rquickjs_host_now_us`, returning microseconds since the unix epoch as an `f64`. It backs `Date.now()`, `new Date()` and `performance`-adjacent internals.

```js
const instance = new WebAssembly.Instance(module, {
  env: { __rquickjs_host_now_us: () => Date.now() * 1000 },
});
```

Time zone data does not exist here, so scripts observe UTC: `getTimezoneOffset()` is always `0`.

### `Runtime::set_max_stack_size` is mandatory

`JS_DEFAULT_STACK_SIZE` is 1MB, which is exactly the size of the wasm shadow stack. The default computation of `stack_top - 1MB` therefore wraps, and stack-overflow detection is silently lost — deep recursion becomes a hard trap instead of a catchable `RangeError`. Always set an explicit limit:

```rust
let runtime = Runtime::new()?;
runtime.set_max_stack_size(256 * 1024);
```

(Defining `__wasi__` would also stop the wrap, but quickjs's `__wasi__` path pins `stack_limit = 0`, disabling stack checking outright. That is why this target patches the sysroot header instead.)

### The sysroot

The build script downloads `wasi-sysroot-24.0.tar.gz` from the [wasi-sdk-24 release](https://github.com/WebAssembly/wasi-sdk/releases/tag/wasi-sdk-24), verifies it against a pinned sha256 (`35172f7d…888f08`), and extracts it into `$CARGO_HOME/rquickjs-wasi-sysroot`. A checksum mismatch is a hard build failure.

One header is then patched in place: `wasi/api.h` guards itself with `#ifndef __wasi__` / `#error`, so that `#error` is commented out. The patch is idempotent.

Set `RQUICKJS_WASM_SYSROOT` to the root of an existing wasi-sysroot to skip the download entirely. The header patch is still applied to it.

### Testing

`wasm-test/` builds the safe API as a cdylib and `wasm-test/runner.ts` drives a battery through it under [deno](https://deno.com), asserting the import list along the way:

```sh
cargo build --manifest-path wasm-test/Cargo.toml --release --target wasm32-unknown-unknown
deno run --allow-read wasm-test/runner.ts
```

## License

This library is licensed under the [MIT License](LICENSE)
