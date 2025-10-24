# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## [0.10.0] - 2025-10-24

### Added

- Allow `rquickjs-core` to build with `no_std` #[#455](https://github.com/DelSkayn/rquickjs/pull/455)
- Add `PromiseHook` bindings #[#453](https://github.com/DelSkayn/rquickjs/pull/453)
- Add `Module::write` options to enable features like `JS_WRITE_OBJ_STRIP_SOURCE` and `JS_WRITE_OBJ_STRIP_DEBUG` #[#443](https://github.com/DelSkayn/rquickjs/pull/443) and #[#518](https://github.com/DelSkayn/rquickjs/pull/518)
- Allow building with [sanitizer](https://doc.rust-lang.org/beta/unstable-book/compiler-flags/sanitizer.html) #[#425](https://github.com/DelSkayn/rquickjs/pull/425)
- Add `set_host_promise_rejection_tracker` to `AsyncRuntime` #[#452](https://github.com/DelSkayn/rquickjs/pull/452)
- Add linux arm64 support to `sys` crate #[#445](https://github.com/DelSkayn/rquickjs/pull/445)
- Implement `Trace` for `Atom` #[#517](https://github.com/DelSkayn/rquickjs/pull/517)
- Add `disable-assertions` feature to disable runtime assertions in quickjs #[#535](https://github.com/DelSkayn/rquickjs/pull/535)
- Add `JS_TAG_SHORT_BIG_INT` tag to support short BigInt #[#458](https://github.com/DelSkayn/rquickjs/pull/458) and #[#519](https://github.com/DelSkayn/rquickjs/pull/519)

### Changed

- Bump MSRV to 1.82 #[#531](https://github.com/DelSkayn/rquickjs/pull/531)
- Update to 2024 edition #[#473](https://github.com/DelSkayn/rquickjs/pull/473)
- Export `DriveFuture` #[#491](https://github.com/DelSkayn/rquickjs/pull/491)
- Switch to `dlopen2` for native module loading #[#513](https://github.com/DelSkayn/rquickjs/pull/513)

### Fixed

- Fix base objects intrinsic not being enabled in async context #[#442](https://github.com/DelSkayn/rquickjs/pull/442)
- Fix namespace resolution for JsLifetime in derive macro #[#429](https://github.com/DelSkayn/rquickjs/pull/429)
- Fix ownership of ctx pointers to be more ergonomic and fix cleanup bugs #[#433](https://github.com/DelSkayn/rquickjs/pull/433)
- Fix rename option in qjs attribute #[#428](https://github.com/DelSkayn/rquickjs/pull/428)
- Strip llvm suffix for `*-pc-windows-gnullvm` target #[#506](https://github.com/DelSkayn/rquickjs/pull/506)

## [0.9.0] - 2025-01-29

### Breaking Changes

- Switching to quickjs-ng from quickjs as base engine #[369](https://github.com/DelSkayn/rquickjs/pull/369)

This change should bring better performances and features, we are aware that it is bigger in intruction sizes.

We would eventually like to support the "original" quickjs engine again in the future, but we use new functions now that are only available in the `-ng` version.
