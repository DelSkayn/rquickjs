# rquickjs-core

[![github](https://img.shields.io/badge/github-delskayn/rquickjs-8da0cb.svg?style=for-the-badge&logo=github)](https://github.com/DelSkayn/rquickjs)
[![crates](https://img.shields.io/crates/v/rquickjs.svg?style=for-the-badge&color=fc8d62&logo=rust)](https://crates.io/crates/rquickjs-core)
[![docs](https://img.shields.io/badge/docs.rs-rquickjs-66c2a5?style=for-the-badge&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K)](https://docs.rs/rquickjs-core)
[![status](https://img.shields.io/github/actions/workflow/status/DelSkayn/rquickjs/ci.yml?branch=master&style=for-the-badge&logo=github-actions&logoColor=white)](https://github.com/DelSkayn/rquickjs/actions?query=workflow%3ARust)

This library is a high level bindings the [QuickJS](https://bellard.org/quickjs/) JavaScript engine.
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
  - Both __Tokio__ and __AsyncStd__ runtimes supported natively
  - Easy integration with almost any async runtime or executor
- Flexible data conversion between Rust and JS
  - Many widely used Rust types can be converted to JS and vice versa
  - `FromJs` and `IntoJs` derive macros can help adapt user defined types too
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
    (Data type which holds refs should implement `HasRefs` trait to get garbage collector works properly)
  - Support for extending defined classes by JS
- Easy bindings using proc macro
  (The interfacing between JS and Rust became much easy with `bind` macro)

## Development status

This bindings is feature complete, mostly stable and ready to use.
The error handling is only thing which may change in the future.
Some experimental features like `parallel` may not works as expected. Use it for your own risk.

## License

This library is licensed under the [MIT License](LICENSE)
