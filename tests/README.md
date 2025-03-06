wasm32-wasip1 testing:

```bash
cargo test --target wasm32-wasip1 --no-run --all

   Compiling rquickjs v0.9.0 (/home/rquickjs)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.72s
  Executable unittests src/lib.rs (target/wasm32-wasip1/debug/deps/native_module-ccc297da8e274ca0.wasm)
  Executable unittests src/lib.rs (target/wasm32-wasip1/debug/deps/rquickjs-7b0f47a051e1e7ac.wasm)
  Executable tests/compile.rs (target/wasm32-wasip1/debug/deps/compile-7bcb05702060ac18.wasm)
  Executable unittests src/main.rs (target/wasm32-wasip1/debug/deps/rquickjs_cli-53ac703b02aa7434.wasm)
  Executable unittests src/lib.rs (target/wasm32-wasip1/debug/deps/rquickjs_core-d8297652eb5516dc.wasm)
  Executable unittests src/lib.rs (target/debug/deps/rquickjs_macro-3a129e36e0157099)
  Executable unittests src/lib.rs (target/wasm32-wasip1/debug/deps/rquickjs_sys-8b4a134ad8d9806d.wasm)

wasmtime target/wasm32-wasip1/debug/deps/native_module-ccc297da8e274ca0.wasm
wasmtime target/wasm32-wasip1/debug/deps/rquickjs-7b0f47a051e1e7ac.wasm
```

Here is the real final solution, dead simple: CARGO_TARGET_WASM32_WASIP1_RUNNER="wasmtime" cargo test --target wasm32-wasip1 --all
