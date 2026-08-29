# QuickJS Tiered JIT Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build an optional, semantically exact, tiered QuickJS JIT in an independent `rquickjs-jit` crate, with transparent interpreter fallback and measured acceleration for compute-heavy JavaScript and `gpui-shell` workloads.

**Architecture:** QuickJS exposes a small versioned internal ABI for owned bytecode snapshots, native entry dispatch, helpers, safe points, and lifecycle notifications. The independent JIT crate owns verification, Cranelift lowering, hotness, background compilation, OSR/deoptimization, executable memory, code caching, and metrics. Tier 0 remains the interpreter, Tier 1 is a frame-compatible baseline compiler, and Tier 2 adds feedback-guided SSA optimization.

**Tech Stack:** Rust 1.87+, C11 QuickJS internals, Cranelift 0.116, platform virtual-memory APIs, Cargo features, Test262, proptest/libFuzzer-style fuzzing, Criterion 0.5 benchmark sampling, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-08-29-quickjs-tiered-jit-design.md`

## Global Constraints

- JavaScript semantics must remain exact; any unproved or unsupported function executes in the existing interpreter.
- The compiler, verifier, tiering policy, cache, executable-memory implementation, metrics, and most tests live in the independent `rquickjs-jit` crate.
- QuickJS/rquickjs changes are limited to a feature-gated, versioned ABI and safe lifecycle bridge; they must not depend on Cranelift.
- Native execution supports macOS, Windows, and Linux on x86-64 and AArch64.
- WebAssembly never enables native JIT execution and retains current interpreter behavior.
- The GPUI foreground thread never waits for background compilation.
- Executable memory is always W^X; no platform may retain RWX pages.
- Compute benchmarks target at least 5x geometric-mean speedup and representative hot kernels target at least 10x.
- Real steady-state `gpui-shell` script workloads target at least 2x; first-window, hot-reload, and P99 script-render latency may not regress by more than 5% under the automatic policy.
- Every performance report includes cold, tier-up, compilation, break-even, steady-state, memory, coverage, hit-rate, and fallback data.

## File map

### Existing files modified

- `Cargo.toml`: add the JIT workspace member and workspace dependencies; the facade only forwards `jit-abi` and never depends on the JIT crate.
- `sys/Cargo.toml`: add the internal `jit-abi` build feature.
- `sys/build.rs`: copy `quickjs-jit.h`, define `CONFIG_JIT_ABI`, and include the ABI in generated bindings.
- `sys/quickjs.bind.h`: include `quickjs-jit.h` only when the ABI feature is enabled.
- `sys/quickjs/quickjs.c`: add narrow snapshot, dispatch, helper, feedback, OSR, and lifetime hooks.
- `sys/quickjs/quickjs.h`: forward-declare only public opaque JIT ABI types needed by embedders.
- `core/Cargo.toml`: forward `jit-abi` to `rquickjs-sys`.
- `core/src/runtime/raw.rs`: attach/detach the backend while holding the runtime lock.
- `core/src/runtime/base.rs`: expose a feature-gated safe owning guard for the low-level backend registration.
- `core/src/runtime.rs`: export JIT ABI lifecycle types under the feature.
- `.github/workflows/ci.yml`: add JIT build/test/platform jobs while retaining non-JIT and WASM jobs.

### New QuickJS/rquickjs adapter files

- `sys/quickjs/quickjs-jit.h`: authoritative C ABI shared by QuickJS and Rust bindings.
- `core/src/runtime/jit.rs`: Rust lifecycle bridge; contains no compiler or policy.

### New independent crate

- `jit/Cargo.toml`: optional Cranelift and platform dependencies.
- `jit/src/lib.rs`: public `JitRuntime`, `Jit`, builder, and exports.
- `jit/src/config.rs`: validated policy and resource limits.
- `jit/src/error.rs`: attachment/setup errors; compile failures remain diagnostics.
- `jit/src/metrics.rs`: atomic counters and immutable metric snapshots.
- `jit/src/abi.rs`: safe wrappers around the versioned C ABI.
- `jit/src/bytecode/{mod,decode,cfg,stack,verify}.rs`: owned snapshot decoding and proof before compilation.
- `jit/src/ir/{mod,types,baseline,optimized,frame_state}.rs`: QuickJS-specific compiler IR and interpreter-state maps.
- `jit/src/compiler/{mod,baseline,optimized,helpers}.rs`: Cranelift lowering and helper declarations.
- `jit/src/runtime/{mod,coordinator,hotness,feedback,install,invalidate,osr}.rs`: tiering state machine and runtime-thread coordination.
- `jit/src/code_cache/{mod,artifact,evict}.rs`: artifact ownership and benefit-aware eviction.
- `jit/src/platform/{mod,linux,macos,windows,unsupported}.rs`: W^X allocation and instruction-cache synchronization.
- `jit/tests/{api,abi,semantics,differential,lifecycle,osr,deopt,platform}.rs`: integration tests grouped by contract.
- `jit/tests/support/mod.rs`: shared runtime-mode, snapshot, synthetic-frame, lifecycle, and differential test harness; each task extends only the helpers it consumes.
- `jit/benches/{micro,algorithms,tiering}.rs`: core benchmark groups.
- `jit/fuzz/fuzz_targets/{snapshot,verifier,differential,frame_state}.rs`: untrusted-input and equivalence fuzzing.
- `benchmarks/scripts/*.js`: checked-in representative JavaScript workloads.
- `benchmarks/Cargo.toml`: non-published benchmark-runner package.
- `benchmarks/run.rs`: reproducible benchmark runner and JSON output.
- `benchmarks/report.rs`: comparison statistics and Markdown/JSON report generation.

---

### Task 1: Independent crate, feature gates, and no-op public API

**Files:**
- Modify: `Cargo.toml`
- Modify: `sys/Cargo.toml`
- Modify: `core/Cargo.toml`
- Create: `jit/Cargo.toml`
- Create: `jit/src/lib.rs`
- Create: `jit/src/config.rs`
- Create: `jit/src/error.rs`
- Create: `jit/src/metrics.rs`
- Create: `jit/tests/api.rs`

**Interfaces:**
- Produces: `JitConfig`, `JitConfigBuilder`, `JitRuntimeBuilder`, `JitRuntime`, `Jit`, `JitMetrics`, and `JitError`.
- Produces feature names: root/core/sys `jit-abi` and crate-local
  `rquickjs-jit/compiler`. The root facade has no `jit` feature because it cannot
  depend back on `rquickjs-jit` without a Cargo dependency cycle.
- Constraint: `rquickjs-jit` depends directly on `rquickjs-core` with `jit-abi`;
  it uses the root `rquickjs` facade only as a dev-dependency for consumer API
  tests. `rquickjs` never depends on `rquickjs-jit`, so the graph is acyclic.

- [ ] **Step 1: Write compile-time API tests**

```rust
// jit/tests/api.rs
use rquickjs::{Context, Runtime};
use rquickjs_jit::{JitConfig, JitError, JitRuntime};

#[test]
fn config_defaults_are_bounded() {
    let cfg = JitConfig::default();
    assert!(cfg.call_threshold() > 0);
    assert!(cfg.loop_threshold() > 0);
    assert!(cfg.max_code_bytes() >= 1024 * 1024);
    assert!(cfg.max_queue_len() > 0);
}

#[test]
fn wrapper_derefs_to_runtime() {
    fn accepts_runtime(_: &Runtime) {}
    let runtime = JitRuntime::builder().build().expect("JIT runtime");
    accepts_runtime(&runtime);
    let context = Context::full(&runtime).expect("context");
    context.with(|ctx| assert_eq!(ctx.eval::<i32, _>("40 + 2").unwrap(), 42));
}

#[test]
fn invalid_limits_are_rejected() {
    let error = JitConfig::builder().max_queue_len(0).build().unwrap_err();
    assert!(matches!(error, JitError::InvalidConfig("max_queue_len")));
}
```

- [ ] **Step 2: Run the API test and verify that the crate is absent**

Run: `cargo test -p rquickjs-jit --test api`

Expected: FAIL because package `rquickjs-jit` does not exist.

- [ ] **Step 3: Add manifests, feature forwarding, and the bounded config API**

Use these exact defaults in `jit/src/config.rs`:

```rust
pub const DEFAULT_CALL_THRESHOLD: u32 = 32;
pub const DEFAULT_LOOP_THRESHOLD: u32 = 56;
pub const DEFAULT_MAX_CODE_BYTES: usize = 16 * 1024 * 1024;
pub const DEFAULT_MAX_QUEUE_LEN: usize = 256;
pub const DEFAULT_WORKERS: usize = 1;
pub const DEFAULT_MAX_COMPILE_ATTEMPTS: u8 = 4;
```

Define exact feature forwarding: root `jit-abi = ["rquickjs-core/jit-abi"]`,
core `jit-abi = ["rquickjs-sys/jit-abi"]`, sys `jit-abi = []`, and JIT crate
`compiler = ["dep:cranelift-codegen", "dep:cranelift-frontend",
"dep:cranelift-module", "dep:cranelift-object", "dep:cranelift-native"]`.

Declare `rquickjs-core = { path = "../core", features = ["jit-abi"] }` as the
runtime dependency and `rquickjs = { path = "..", features = ["jit-abi"] }`
only under `[dev-dependencies]`. The facade re-exports the same `Runtime` type,
so the consumer-facing compile test above proves interoperability without a
production dependency cycle.

Pin Cranelift `codegen`, `frontend`, `module`, `object`, and `native` to the same
`0.116` release line. Make them optional dependencies behind crate feature
`compiler`, and declare them only for non-WASM targets so a WASM dependency
graph contains no native backend.

Implement `JitRuntime` initially as an ordinary `Runtime` plus a disabled `Jit`
guard. On WASM, `JitRuntimeBuilder::build()` must still construct an interpreted
runtime and expose `metrics().native_enabled() == false`; explicit
`Jit::require_native()` returns `JitError::UnsupportedPlatform`.

- [ ] **Step 4: Run API and non-JIT compatibility checks**

Run: `cargo test -p rquickjs-jit --test api && cargo test -p rquickjs-core --lib && cargo check --workspace`

Expected: PASS. `cargo tree -p rquickjs-core` must not contain a Cranelift crate.

- [ ] **Step 5: Commit the independent crate skeleton**

```bash
git add Cargo.toml sys/Cargo.toml core/Cargo.toml jit
git commit -m "feat(jit): add independent JIT crate API"
```

### Task 2: Versioned C ABI and safe runtime attachment

**Files:**
- Create: `sys/quickjs/quickjs-jit.h`
- Modify: `sys/quickjs/quickjs.c`
- Modify: `sys/quickjs.bind.h`
- Modify: `sys/build.rs`
- Modify: `scripts/gen-bindings.sh`
- Modify (generated): `sys/src/bindings/x86_64-unknown-linux-gnu.rs`
- Modify (generated): `sys/src/bindings/aarch64-unknown-linux-gnu.rs`
- Modify (generated): `sys/src/bindings/x86_64-unknown-linux-musl.rs`
- Modify (generated): `sys/src/bindings/aarch64-unknown-linux-musl.rs`
- Modify (generated): `sys/src/bindings/x86_64-apple-darwin.rs`
- Modify (generated): `sys/src/bindings/aarch64-apple-darwin.rs`
- Modify (generated): `sys/src/bindings/x86_64-pc-windows-gnu.rs`
- Modify (generated): `sys/src/bindings/x86_64-pc-windows-msvc.rs`
- Modify (generated): `sys/src/bindings/aarch64-pc-windows-msvc.rs`
- Create: `core/src/runtime/jit.rs`
- Modify: `core/src/runtime/raw.rs`
- Modify: `core/src/runtime/base.rs`
- Modify: `core/src/runtime.rs`
- Create: `jit/src/abi.rs`
- Create: `jit/tests/abi.rs`
- Create: `jit/tests/support/mod.rs`

**Interfaces:**
- Produces C constants `QJSJIT_ABI_MAJOR = 1`, `QJSJIT_ABI_MINOR = 0`.
- Produces `JSJitABIInfo`, `JSJitBackendVTable`, `JSJitFunctionId`, `JS_SetJitBackend`, `JS_GetJitABIInfo`.
- Produces Rust `unsafe trait JitBackend` and `RuntimeJitGuard` in rquickjs-core.
- Consumes the no-op `Jit` owner from Task 1.

- [ ] **Step 1: Write ABI layout and attach/detach tests**

```rust
// jit/tests/abi.rs
use rquickjs_jit::abi::{AbiInfo, ABI_MAJOR, ABI_MINOR};

#[test]
fn linked_abi_matches_rust_contract() {
    let info = AbiInfo::linked().expect("ABI info");
    assert_eq!(info.major(), ABI_MAJOR);
    assert!(info.minor() >= ABI_MINOR);
    assert_eq!(info.pointer_width(), usize::BITS as u8);
    assert_eq!(info.little_endian(), cfg!(target_endian = "little"));
}

#[test]
fn backend_is_detached_before_runtime_drop() {
    let events = rquickjs_jit::test_support::record_lifecycle();
    {
        let _runtime = events.runtime();
    }
    assert_eq!(events.take(), ["attach", "detach", "backend_drop", "runtime_drop"]);
}
```

- [ ] **Step 2: Run the ABI test and verify missing symbols**

Run: `cargo test -p rquickjs-jit --test abi`

Expected: link or compile failure for `JS_GetJitABIInfo` and `RuntimeJitGuard`.

- [ ] **Step 3: Define the minimal ABI**

The header must use fixed-width fields, size/version prefixes, and opaque
pointers. Define this public shape, extending only at the tail:

```c
#define QJSJIT_ABI_MAJOR 1u
#define QJSJIT_ABI_MINOR 0u

typedef struct JSJitABIInfo {
    uint32_t struct_size;
    uint16_t major;
    uint16_t minor;
    uint8_t pointer_width;
    uint8_t little_endian;
    uint16_t value_size;
    uint64_t source_revision;
    uint64_t opcode_fingerprint;
    uint64_t value_layout_fingerprint;
    uint64_t build_feature_flags;
    uint64_t build_fingerprint;
} JSJitABIInfo;

typedef struct JSJitBackendVTable {
    uint32_t struct_size;
    uint32_t (*record_hot)(void *opaque, const JSJitHotEvent *event);
    void (*submit_snapshot)(void *opaque, JSJitFunctionSnapshot *snapshot);
    JSJitEntryHandle (*acquire_entry)(void *opaque, uint64_t id,
                                      uint64_t generation, uint32_t pc);
    void (*release_entry)(void *opaque, JSJitEntryHandle entry);
    void (*runtime_detach)(void *opaque, JSRuntime *rt);
    void (*function_retire)(void *opaque, uint64_t id, uint64_t generation);
    size_t (*memory_used)(void *opaque);
} JSJitBackendVTable;

int JS_GetJitABIInfo(JSJitABIInfo *out);
int JS_SetJitBackend(JSRuntime *rt, const JSJitBackendVTable *vtable, void *opaque);
```

Store the vtable and opaque pointer in `JSRuntime` only under `CONFIG_JIT_ABI`.
Reject double attachment and mismatched `struct_size`. Detach is idempotent.
Define every event, snapshot, entry handle, and callback argument in the same
header with `struct_size` and fixed-width fields. Add layout fingerprints for
all ABI structs; do not export private C offsets.

Add `jit-abi` to the binding-generation feature set in
`scripts/gen-bindings.sh`, regenerate the representative Linux, Darwin, and
Windows bindings, and copy them to the nine supported target files listed
above. A test compares bindgen output with each bundled file's JIT declarations
after normalizing target-specific `size_t` aliases.

- [ ] **Step 4: Implement the Rust guard without exposing `JSRuntime` publicly**

`core/src/runtime/jit.rs` owns the backend box and registers its generated
vtable while `RawRuntime` is locked. `RuntimeJitGuard::drop` calls
`JS_SetJitBackend(rt, NULL, NULL)` before freeing the backend. `RawRuntime::drop`
asserts in debug builds that no backend remains.

Create `jit/tests/support/mod.rs` with `LifecycleRecorder`, backed by
`Arc<Mutex<Vec<&'static str>>>`. Its test backend records `attach`, `detach`, and
`backend_drop`; a test-only runtime drop probe records `runtime_drop`.
`record_lifecycle() -> LifecycleRecorder` is test support, not public API.

Implement and test `Jit::attach(&Runtime, JitConfig) -> Result<Jit, JitError>`.
The returned `Jit` owns `RuntimeJitGuard`; configuration includes a structured
diagnostic callback and metrics-observer callback. Add negative ABI fixtures
for source revision, opcode fingerprint, value layout, feature flags, pointer
width, endianness, and every ABI-structure layout fingerprint. Each fixture
must fail attachment before its vtable is stored.

- [ ] **Step 5: Run bindings and lifecycle verification**

Run: `cargo test -p rquickjs-jit --test abi --features rquickjs-core/bindgen`

Run: `cargo test -p rquickjs-core runtime::test --features jit-abi`

Expected: PASS in generated-binding and bundled-binding configurations.

- [ ] **Step 6: Commit the ABI bridge**

```bash
git add sys core/src/runtime jit/src/abi.rs jit/tests/abi.rs
git commit -m "feat(jit): add versioned QuickJS backend ABI"
```

### Task 3: Owned function snapshots and generated opcode metadata

**Files:**
- Modify: `sys/quickjs/quickjs-jit.h`
- Modify: `sys/quickjs/quickjs.c`
- Modify: `sys/build.rs`
- Create: `jit/src/bytecode/mod.rs`
- Create: `jit/src/bytecode/decode.rs`
- Create: `jit/src/bytecode/cfg.rs`
- Create: `jit/src/bytecode/stack.rs`
- Create: `jit/src/bytecode/verify.rs`
- Create: `jit/tests/snapshot.rs`
- Create: `jit/tests/verifier.rs`
- Modify: `jit/tests/support/mod.rs`

**Interfaces:**
- Produces `JSJitFunctionSnapshot`, `JSJitOpcodeInfo`, `JSJitSnapshotStatus`, and `JS_JitSnapshotFunction`/`JS_JitFreeSnapshot`.
- Produces Rust `CompileSnapshot: Send + Sync`, runtime-thread-only
  `RuntimeConstants: !Send`, `Instruction`, `ControlFlowGraph`,
  `VerifiedFunction`, and `VerifyError`.
- Consumes the ABI fingerprint from Task 2.

- [ ] **Step 1: Write snapshot ownership and decode tests**

```rust
#[test]
fn snapshot_survives_source_function_collection() {
    let fixture = SnapshotFixture::compile("function f(a) { return a + 1 } f");
    let snapshot = fixture.snapshot();
    drop(fixture);
    assert!(snapshot.bytecode().len() >= 4);
    assert_eq!(snapshot.arg_count(), 1);
    assert!(snapshot.decode().unwrap().iter().any(|i| i.opcode().name() == "add"));
}

#[test]
fn decoder_rejects_truncated_operand() {
    let bytes = [opcode::PUSH_I32, 1, 2];
    assert_eq!(decode_raw(&bytes).unwrap_err(), DecodeError::Truncated { pc: 0, size: 5 });
}
```

- [ ] **Step 2: Run tests and verify the snapshot API is absent**

Run: `cargo test -p rquickjs-jit --test snapshot --test verifier`

Expected: FAIL for missing snapshot and decoder types.

- [ ] **Step 3: Generate opcode metadata from QuickJS's authoritative table**

Use `quickjs-opcode.h` through C macro expansion to export an immutable table
containing opcode number, encoded size, pop count, push count, and operand
format. Do not duplicate numeric opcode values in Rust. `sys/build.rs` emits a
fingerprint over the table and bindgen exposes the table accessor.

- [ ] **Step 4: Implement owned snapshots**

Put bytecode bytes, scalar metadata, exception/source maps, and copied constant
descriptors in `CompileSnapshot`, one C allocation owned by
`JS_JitFreeSnapshot`. Keep heap constants in a separate `RuntimeConstants`
table owned and accessed only on the runtime thread. Compile-time constant
descriptors contain tag/kind/index data but no heap pointer or callback. Native
code resolves a constant index through the runtime helper table at execution.
Add `static_assert_send_sync::<CompileSnapshot>()` and
`static_assert_not_impl_any!(RuntimeConstants: Send, Sync)` tests.
Reject generators, async functions, `eval`, and `with` with categorized status
codes at this stage.

Extend test support with `SnapshotFixture::compile(source: &str)`,
`SnapshotFixture::snapshot() -> CompileSnapshot`, and `decode_raw(&[u8])`.
The fixture owns its runtime, context, and function until the snapshot takes
independent ownership; dropping it forces QuickJS GC before the snapshot is
decoded.

- [ ] **Step 5: Implement decoder, CFG, stack proof, and verifier**

The verifier must reject unknown/truncated opcodes, branch targets inside an
instruction, underflow, inconsistent merge heights, out-of-range local/arg/
closure/constant indices, unsupported exception regions, and resource limits.
Return the first deterministic `VerifyError { pc, kind }`.

Track conservative slot kinds (`Tagged`, `Int32`, `Float64`, `CatchOffset`, and
`Uninitialized`) and reject incompatible merge kinds unless an explicit boxing
join converts both inputs to `Tagged`. Verify OSR points are loop headers with a
complete live-slot map; verify deopt points are instruction boundaries with a
complete pre/post-side-effect state. Add malformed merge-kind, OSR, and deopt
fixtures that fail before IR construction.

Enforce distinct configured limits for snapshot bytes, decoded instructions,
basic blocks, metadata bytes, and verifier work units. Limit failures identify
the exhausted resource rather than returning a generic invalid-bytecode error.

- [ ] **Step 6: Run focused and property tests**

Run: `cargo test -p rquickjs-jit --test snapshot --test verifier`

Run: `cargo test -p rquickjs-jit bytecode:: --lib`

Expected: PASS, including a property test that decoding arbitrary byte arrays
never panics and either consumes the complete buffer or returns an error.

- [ ] **Step 7: Commit snapshot and verification**

```bash
git add sys jit/src/bytecode jit/tests/snapshot.rs jit/tests/verifier.rs
git commit -m "feat(jit): snapshot and verify QuickJS bytecode"
```

### Task 4: C execution boundary, function registry, and resumable exits

**Files:**
- Modify: `sys/quickjs/quickjs-jit.h`
- Modify: `sys/quickjs/quickjs.c`
- Modify: `core/src/runtime/jit.rs`
- Modify: `core/src/runtime/raw.rs`
- Modify: `jit/src/abi.rs`
- Create: `jit/tests/native_boundary.rs`
- Modify: `jit/tests/support/mod.rs`

**Interfaces:**
- Produces `JSJitExecFrame`, `JSJitExitKind`, `JSJitExit`, `JSJitEntryFn`, and
  the authoritative interpreter resume contract.
- Produces function IDs/generations tied to `JSFunctionBytecode` lifetime, not
  closure-object addresses.
- Consumes the vtable and snapshot ABI from Tasks 2-3; it does not consume
  Cranelift or executable-memory code.

- [ ] **Step 1: Write hand-authored native-boundary tests**

Attach a test backend whose `acquire_entry` returns one of four ordinary Rust
`extern "C"` functions selected by function ID:

```rust
unsafe extern "C" fn native_done(frame: *mut JSJitExecFrame) -> JSJitExit {
    (*frame).result = JS_NewInt32((*frame).ctx, 42);
    JSJitExit::done()
}

unsafe extern "C" fn native_deopt(frame: *mut JSJitExecFrame) -> JSJitExit {
    JSJitExit::resume((*frame).bytecode_start.add((*frame).test_resume_pc as usize))
}
```

Add tests for native return, a native helper that throws and is caught by JS,
an uncatchable interrupt, and deopt resuming after a side-effect counter. The
deopt test must prove the counter increments once, not twice.

- [ ] **Step 2: Run the boundary test and verify native entries are unreachable**

Run: `cargo test -p rquickjs-jit --test native_boundary`

Expected: FAIL because `JSJitExecFrame` and native entry dispatch do not exist.

- [ ] **Step 3: Define the authoritative frame and exit ABI**

```c
typedef enum JSJitExitKind {
    JS_JIT_EXIT_DONE = 0,
    JS_JIT_EXIT_EXCEPTION = 1,
    JS_JIT_EXIT_INTERRUPT = 2,
    JS_JIT_EXIT_DEOPT = 3,
    JS_JIT_EXIT_RETRY_INTERPRETER = 4
} JSJitExitKind;

typedef struct JSJitExecFrame {
    uint32_t struct_size;
    uint32_t flags;
    JSRuntime *rt;
    JSContext *ctx;
    uint64_t function_id;
    uint64_t generation;
    JSValueConst *arg_buf;
    JSValue *var_buf;
    JSValue *stack_base;
    JSValue *stack_top;
    const uint8_t *bytecode_start;
    const uint8_t *pc;
    JSValue result;
    JSJitEntryHandle entry;
} JSJitExecFrame;

typedef struct JSJitExit {
    uint32_t kind;
    uint32_t reserved;
    const uint8_t *resume_pc;
    JSValue *resume_stack_top;
} JSJitExit;

typedef JSJitExit (*JSJitEntryFn)(JSJitExecFrame *frame);
```

`JSJitEntryHandle` contains entry function plus opaque pin token. Pointer fields
are runtime-thread-only and never enter a compile snapshot.

- [ ] **Step 4: Refactor `JS_CallInternal` around an explicit resumable frame**

Keep allocation, current-stack-frame linkage, catch/finally search, backtrace,
and cleanup in C. At function entry and verified OSR polls, acquire an entry,
populate `JSJitExecFrame`, call it, then switch locally on `JSJitExitKind`:

- `DONE` enters the existing `done` cleanup with `frame.result`;
- `EXCEPTION` and `INTERRUPT` enter the existing `exception` label with
  authoritative PC/SP;
- `DEOPT` validates `resume_pc` and `resume_stack_top`, assigns local PC/SP, and
  enters `restart`;
- `RETRY_INTERPRETER` restarts at the original entry PC before side effects.

Release the entry pin on every branch before interpreter cleanup. Native code
never calls or jumps to a C-local label.

- [ ] **Step 5: Add bytecode identity and generation registry**

Assign identity on `JSFunctionBytecode`, share it across closure objects, and
emit retirement only from bytecode finalization. Store retained runtime
constants in the registry and release them on the runtime thread. Provide an
explicit `JS_JitInvalidateFunction` for reload integrations; address reuse can
never reuse a generation.

- [ ] **Step 6: Bind backend lifetime to `RawRuntime`**

Store the backend allocation in `RawRuntime`'s shared state, keyed by an attach
token. Dropping `Jit` detaches the token and makes surviving `Runtime` clones
interpreter-only. `RawRuntime::drop` forcibly detaches any remaining token and
waits for pins before `JS_FreeRuntime`. Test cloned `Runtime`, live `Context`,
guard drop while idle, queued work at drop, and backend release ordering.

- [ ] **Step 7: Run native-boundary and interpreter regression tests**

Run: `cargo test -p rquickjs-jit --test native_boundary --test abi --test lifecycle`

Run: `cargo test -p rquickjs-core --lib --features jit-abi`

Expected: all PASS, including caught throw, interrupt, exact deopt resume, and
interpreter execution after guard drop.

- [ ] **Step 8: Commit the execution boundary**

```bash
git add sys/quickjs core/src/runtime jit/src/abi.rs jit/tests/native_boundary.rs jit/tests/support/mod.rs
git commit -m "feat(jit): add resumable QuickJS native boundary"
```

### Task 5: Runtime coordinator, hotness state, and deterministic mock compiler

**Files:**
- Create: `jit/src/runtime/mod.rs`
- Create: `jit/src/runtime/coordinator.rs`
- Create: `jit/src/runtime/hotness.rs`
- Create: `jit/src/runtime/install.rs`
- Create: `jit/src/runtime/invalidate.rs`
- Create: `jit/src/compiler/mock.rs`
- Create: `jit/src/code_cache/mod.rs`
- Create: `jit/src/code_cache/artifact.rs`
- Create: `jit/src/code_cache/evict.rs`
- Modify: `jit/src/metrics.rs`
- Create: `jit/tests/lifecycle.rs`
- Modify: `jit/tests/support/mod.rs`

**Interfaces:**
- Produces `FunctionKey { id, generation }`, `Tier`, `CompileState`, `CompileRequest`, `CompiledArtifact`, and `Coordinator`.
- Produces `Compiler` trait: `fn compile(&self, request: CompileRequest) -> Result<CompiledArtifact, CompileFailure>`.
- Consumes verified owned snapshots from Task 3.

- [ ] **Step 1: Write state-machine tests with a fake compiler**

```rust
#[test]
fn stale_result_is_never_installed() {
    let h = Harness::new();
    let key = FunctionKey::new(7, 3);
    h.queue(key);
    h.retire(key);
    h.complete(key, CompiledArtifact::fake(Tier::Baseline));
    assert_eq!(h.state(key), CompileState::Retired);
    assert_eq!(h.metrics().stale_results, 1);
    assert_eq!(h.metrics().installed, 0);
}

#[test]
fn repeated_failures_blacklist_only_the_generation() {
    let h = Harness::with_max_attempts(4);
    let key = FunctionKey::new(9, 1);
    for _ in 0..4 { h.fail(key, CompileFailure::UnsupportedOpcode); }
    assert_eq!(h.state(key), CompileState::Blacklisted);
    assert_eq!(h.state(FunctionKey::new(9, 2)), CompileState::Cold);
}
```

- [ ] **Step 2: Verify failure before coordinator implementation**

Run: `cargo test -p rquickjs-jit --test lifecycle`

Expected: FAIL for missing coordinator types.

- [ ] **Step 3: Implement the transition table and bounded queues**

Use explicit transitions only:

```rust
pub enum CompileState {
    Cold,
    Queued(Tier),
    Compiling(Tier),
    Ready(Tier),
    Installed(Tier),
    Backoff { attempts: u8, retry_after: u64 },
    Blacklisted,
    Retired,
}
```

No worker mutates runtime state directly. Workers send `CompileCompletion` to a
bounded channel; `Coordinator::drain_completions` is called only under the
runtime lock. Use generation checks before every transition to `Ready` or
`Installed`.

Define `CompileCompletion { key: FunctionKey, requested_tier: Tier, result:
Result<CompiledArtifact, CompileFailure> }` and a closed `CompileFailure` enum.
Define coordinator methods `queue(key, tier, snapshot)`, `begin_next()`,
`complete(completion)`, `retire(key)`, and `state(key)`. The test-only
`FakeCompiler` lives in `jit/src/compiler/mock.rs` and completes work only when
its harness releases it.

Extend test support with a deterministic `Harness` and manually completed
`FakeCompiler`. Define exactly `new`, `with_max_attempts`, `queue`, `retire`,
`complete`, `fail`, `state`, and `metrics`. `CompiledArtifact::fake` allocates no native
memory and is restricted to coordinator tests.

- [ ] **Step 4: Implement cache ownership and active-entry pinning**

`CompiledArtifact` owns code allocation, relocations, stack maps, frame states,
dependencies, tier, and benefit counters. Eviction cannot remove artifacts with
nonzero execution pins. Tests use a 3-artifact limit and assert benefit/recency
ordering deterministically.

Index artifacts by `ArtifactKey { runtime_id, function_id, generation, tier,
target_isa, cpu_features, abi_fingerprint }`. Add collision tests varying one
field at a time. When Tier 2 replaces Tier 1, pin Tier 1 as its deopt target;
release Tier 1 only after Tier 2 is invalidated/evicted and all Tier 2 execution
pins and deopt references reach zero.

- [ ] **Step 5: Run lifecycle tests under normal and Loom-style schedules**

Run: `cargo test -p rquickjs-jit --test lifecycle`

Run: `cargo test -p rquickjs-jit runtime:: --lib -- --test-threads=1`

Expected: PASS with queue saturation, stale completion, shutdown, retry, and
active-code eviction tests.

- [ ] **Step 6: Commit the coordinator**

```bash
git add jit/src/runtime jit/src/code_cache jit/src/metrics.rs jit/tests/lifecycle.rs
git commit -m "feat(jit): coordinate tiering and code lifetime"
```

### Task 6: Cross-platform W^X code allocator

**Files:**
- Create: `jit/src/platform/mod.rs`
- Create: `jit/src/platform/linux.rs`
- Create: `jit/src/platform/macos.rs`
- Create: `jit/src/platform/windows.rs`
- Create: `jit/src/platform/unsupported.rs`
- Create: `jit/tests/platform.rs`
- Create: `jit/tests/support/host_asm.rs`
- Modify: `jit/Cargo.toml`

**Interfaces:**
- Produces `CodeAllocator`, `WritableCode`, `ExecutableCode`, and `CodeMemoryError`.
- `WritableCode::apply_relocations(&mut self, &[Relocation])` validates and
  writes relocations; `WritableCode::publish(self) ->
  Result<ExecutableCode, CodeMemoryError>` consumes writable access permanently.
- Consumes no QuickJS state.

- [ ] **Step 1: Write allocator contract tests**

```rust
#[test]
fn publish_is_one_way_and_code_executes() {
    let allocator = CodeAllocator::for_host().unwrap();
    let mut writable = allocator.allocate(4096).unwrap();
    host_asm::write_return_42(&mut writable).unwrap();
    let executable = writable.publish().unwrap();
    let result = unsafe { executable.call0_i32() };
    assert_eq!(result, 42);
    assert!(!executable.is_writable());
}

#[test]
fn quota_is_enforced_before_mapping() {
    let allocator = CodeAllocator::with_limit(4096).unwrap();
    let _first = allocator.allocate(4096).unwrap();
    assert!(matches!(allocator.allocate(1), Err(CodeMemoryError::LimitExceeded)));
}
```

`host_asm::write_return_42` writes `b8 2a 00 00 00 c3` on x86-64 and
`mov w0,#42; ret` (`40 05 80 52 c0 03 5f d6`, little-endian) on AArch64.
`ExecutableCode::call0_i32` checks the entry offset, pins the allocation, casts
to `unsafe extern "C" fn() -> i32`, calls it, and releases the pin.

- [ ] **Step 2: Run the platform test and verify missing allocator**

Run: `cargo test -p rquickjs-jit --test platform`

Expected: FAIL for missing platform module.

- [ ] **Step 3: Implement Linux and Windows publishing**

Linux uses `mmap(PROT_READ | PROT_WRITE)` then `mprotect(PROT_READ | PROT_EXEC)`.
Windows uses `VirtualAlloc(PAGE_READWRITE)`, `VirtualProtect(PAGE_EXECUTE_READ)`,
`FlushInstructionCache`, and CFG-valid call-target handling. Every OS error
includes operation and raw error code.

On Linux/AArch64, call the target-supported instruction-cache clear primitive
over the exact published range before entry; test self-modifying two-version
publication across threads to catch missing cache synchronization. Validate
every indirect target offset is aligned, inside the published allocation, and
declared in artifact metadata before CFG registration or entry acquisition.

- [ ] **Step 4: Implement the macOS process code heap**

Use one process-global `MAP_JIT` region, suballocate page-aligned spans, and
synchronize instruction cache. Expose
`MacJitMode::ThreadWriteProtect` (default, using
`pthread_jit_write_protect_np`) and
`MacJitMode::AllowListCallback(unsafe extern "C" fn(*mut c_void) -> c_int)`
for signed embedders that must route writes through
`pthread_jit_write_with_callback_np`. Never guess which policy an embedder
uses. Maintain runtime owner IDs and logical quotas. Return
`MissingEntitlement` or `WriteCallbackRejected` without aborting, disable native
installation for that runtime, and retain interpreter execution.

- [ ] **Step 5: Add unsupported-target behavior**

For `target_family = "wasm"` and non-x86-64/AArch64 native targets,
`CodeAllocator::for_host()` returns `UnsupportedPlatform`; building rquickjs
without `jit` remains unaffected.

- [ ] **Step 6: Run native tests and cross-target checks**

Run: `cargo test -p rquickjs-jit --test platform`

Run: `cargo check -p rquickjs-jit --target wasm32-wasip1`

Run on platform CI: `cargo test -p rquickjs-jit --test platform --release`

Expected: native test PASS; WASM check PASS with no Cranelift native backend or
executable-memory syscall linked.

Fault-injection variants deny mapping, protection change, cache flush, CFG
registration, and macOS write-protect transitions. Each returns a categorized
error, disables native installation for the affected runtime, and leaves an
ordinary interpreted `1 + 1` evaluation working.

- [ ] **Step 7: Commit platform memory support**

```bash
git add jit/Cargo.toml jit/src/platform jit/tests/platform.rs
git commit -m "feat(jit): allocate W^X code on desktop platforms"
```

### Task 7: Tier 1 Cranelift compiler for pure frame operations

**Files:**
- Create: `jit/src/ir/mod.rs`
- Create: `jit/src/ir/types.rs`
- Create: `jit/src/ir/baseline.rs`
- Create: `jit/src/ir/frame_state.rs`
- Create: `jit/src/compiler/mod.rs`
- Create: `jit/src/compiler/baseline.rs`
- Create: `jit/src/compiler/helpers.rs`
- Modify: `jit/Cargo.toml`
- Create: `jit/tests/baseline.rs`
- Modify: `jit/tests/support/mod.rs`

**Interfaces:**
- Produces machine entry signature `extern "C" fn(*mut JSJitExecFrame) -> JSJitExit`.
- Produces `BaselineCompiler::host().compile(&VerifiedFunction) ->
  Result<RelocatableCode, CompileFailure>`; explicit cross-target compilation
  uses `BaselineCompiler::new(TargetIsa)`.
- Consumes the Task 4 execution-frame ABI, verified CFG, and W^X allocator.

- [ ] **Step 1: Write end-to-end compiler tests against synthetic frames**

```rust
#[test]
fn compiles_loop_and_integer_arithmetic() {
    let f = verified("function sum(n) { let s=0; for(let i=0;i<n;i++) s+=i; return s }");
    let code = BaselineCompiler::host().compile(&f).unwrap().publish().unwrap();
    assert_eq!(SyntheticFrame::call_i32(&code, &[100]), 4950);
}

#[test]
fn integer_overflow_takes_number_slow_path() {
    let f = verified("function add(a,b) { return a+b }");
    let result = SyntheticFrame::call(&compile(f), &[i32::MAX.into(), 1.into()]);
    assert_eq!(result.as_f64(), i32::MAX as f64 + 1.0);
}
```

- [ ] **Step 2: Run baseline tests and verify compiler is absent**

Run: `cargo test -p rquickjs-jit --test baseline`

Expected: FAIL for missing baseline compiler.

- [ ] **Step 3: Define compact baseline IR and frame maps**

Represent constants, frame loads/stores, stack moves, branches, checked integer
ops, floating ops, helper calls, return, exception exit, interrupt poll, and OSR
labels. Every IR instruction stores source bytecode PC. Every helper/exit has a
`FrameStateId` that names the live argument/local/stack slots.

Extend test support with `verified(source) -> VerifiedFunction`,
`compile(VerifiedFunction) -> ExecutableCode`, and `SyntheticFrame`. The
synthetic frame stores tagged values in `Vec<JSValueRepr>` and supplies only
numeric slow-path helpers; object helpers panic so these tests cannot claim
object semantics accidentally.

- [ ] **Step 4: Lower pure opcodes through Cranelift**

Cover constants (`undefined`, `null`, booleans, i32), stack permutations,
argument/local loads and stores, `goto`, boolean branches, return, integer and
floating add/sub/mul/div/mod, comparisons, bit operations, and `nop`. Use target
pointer type from the ISA; never hard-code I64 pointers.

Insert interrupt/safepoint polls at function entry, every backedge, before each
call or allocation helper, and at least every 1,024 emitted source bytecodes in
straight-line regions. Add a generated 4,096-operation straight-line fixture
whose interrupt handler must fire before function return.

- [ ] **Step 5: Publish through `CodeAllocator`, not `JITModule`**

Emit relocatable bytes and declared helper relocations, apply them while
writable, publish once, and retain unwind/stack/frame metadata in the artifact.

- [ ] **Step 6: Run compiler tests on x86-64 and AArch64 CI**

Run: `cargo test -p rquickjs-jit --test baseline --release`

Expected: PASS with identical synthetic-frame results on every native runner.

- [ ] **Step 7: Commit Tier 1 pure compilation**

```bash
git add jit/Cargo.toml jit/src/ir jit/src/compiler jit/tests/baseline.rs
git commit -m "feat(jit): compile baseline QuickJS control flow"
```

### Task 8: QuickJS helper ABI and exact value ownership

**Files:**
- Modify: `sys/quickjs/quickjs-jit.h`
- Create: `sys/quickjs/quickjs-jit-helpers.h`
- Modify: `sys/quickjs/quickjs.c`
- Modify: `jit/src/abi.rs`
- Modify: `jit/src/compiler/helpers.rs`
- Create: `jit/tests/semantics.rs`
- Modify: `jit/tests/support/mod.rs`

**Interfaces:**
- Produces `JSJitRuntimeAPI` and a generated, versioned helper table.
- Consumes Task 4 frames/exit handling, native entry functions from Task 7, and
  coordinator artifacts from Task 5.

- [ ] **Step 1: Write forced-Tier-1 semantic tests**

```rust
#[test]
fn compiled_values_have_interpreter_ownership() {
    differential("function f(o) { let x=o; return [x,x] }", "f({a:1})").assert_same();
}

#[test]
fn thrown_getter_is_caught_at_the_same_handler() {
    differential(
        "function f(o){ try { return o.x } catch(e) { return e.message } }",
        "f({get x(){throw new Error('boom')}})",
    ).force_baseline().assert_same();
}

#[test]
fn interrupt_stops_compiled_loop() {
    forced_baseline("function f(){for(;;){}} f()")
        .interrupt_after(100)
        .assert_uncatchable_interrupt();
}
```

- [ ] **Step 2: Run semantics and verify interpreter dispatch never enters code**

Run: `cargo test -p rquickjs-jit --test semantics`

Expected: FAIL because compiled entries have no QuickJS helper table.

- [ ] **Step 3: Connect compiled artifacts to the proven native boundary**

Return published artifact entries from the Task 4 `acquire_entry` callback.
Populate `frame.runtime_api` before entry, keep the Task 4 execution pin, and
let the already-tested C exit switch own DONE/EXCEPTION/INTERRUPT/DEOPT cleanup.

- [ ] **Step 4: Implement ownership-audited helpers**

Expose helpers for dup/free, conversion, add slow path, comparison, property
get/set, JS call, allocation, and interrupt polling. Document each argument as
borrowed/consumed and each result as owned/exception. Add debug counters around
dup/free and assert differential balance after each test runtime drops.

Define helper signatures once in `quickjs-jit-helpers.h` with an X-macro table;
include it from QuickJS C, bindgen input, and a Rust build-time generator used
for Cranelift declarations. Test helper count, name, ABI type sequence,
ownership flags, throwing/allocating flags, and version across all three views.
Every debug helper validates runtime ID, function generation, frame cookie, and
stack-map ID before touching a value.

- [ ] **Step 5: Implement bytecode-position and exception transfer**

Before every throwing helper, store the bytecode PC and stack pointer in the
frame. On exception, return `JS_JIT_EXIT_EXCEPTION`; Task 4's local C exit
switch enters the existing catch/finally search. Rust never invokes a C-local
label.

Extend test support with `differential(definition, expression)`,
`forced_baseline(source)`, and `Run`. Each mode uses a fresh runtime, serializes
results with one checked-in canonicalizer, records host events/exceptions, and
exposes JIT metrics. `Run::assert_same` compares canonical result, exception,
event order, and final ownership counters.

At every helper, interrupt, OSR, and exception map location, stress mode forces
allocation and cycle collection before and after the operation. Tier 1 keeps
owning `JSValue`s in C-visible frame slots across all allocating/reentrant
helpers; stack maps are deopt/debug metadata, not GC roots. Publication rejects
an artifact if a GC-capable instruction lacks a complete live-slot map.

- [ ] **Step 6: Run semantics, leak dumps, and sanitizer subset**

Run: `cargo test -p rquickjs-jit --test semantics --features rquickjs-core/dump-leaks`

Run: `RUSTFLAGS=-Zsanitizer=address cargo +nightly test -p rquickjs-jit --test semantics -Zbuild-std --target x86_64-unknown-linux-gnu`

Expected: PASS, zero leaked test values, no sanitizer findings.

- [ ] **Step 7: Prove the gpui-shell call surface compiles early**

Add `jit/tests/gpui_shell_surface.rs`, a compile-and-run adapter that mirrors
the current shell's runtime creation, context creation, module loader, async
promise driving, interrupt handler, and runtime teardown against `JitRuntime`.
Also run a read-only inventory:

Run: `rg -n "JsRuntime|Runtime::|Context::|set_loader|set_interrupt_handler|execute_pending_job" ../gpui-component/crates/shell/src`

Record every discovered call site in the test as a named compile fixture. This
is the early compatibility gate; Task 15 applies and benchmarks the real sibling
patch once its directory is writable.

- [ ] **Step 8: Commit native dispatch and helpers**

```bash
git add sys/quickjs jit/src/abi.rs jit/src/compiler/helpers.rs jit/tests/semantics.rs jit/tests/gpui_shell_surface.rs
git commit -m "feat(jit): execute baseline code through QuickJS frames"
```

### Task 9: Complete Tier 1 eligible opcode families

**Files:**
- Modify: `jit/src/bytecode/verify.rs`
- Create: `jit/src/bytecode/policy.rs`
- Create: `jit/build.rs`
- Modify: `jit/src/ir/baseline.rs`
- Modify: `jit/src/compiler/baseline.rs`
- Modify: `jit/src/compiler/helpers.rs`
- Modify: `sys/quickjs/quickjs.c`
- Create: `jit/tests/opcodes.rs`
- Create: `jit/tests/differential.rs`
- Create: `jit/tests/fixtures/tier1-programs.txt`
- Create: `jit/tests/fixtures/opcode-cases.json`

**Interfaces:**
- Produces `enum Tier1Policy { Native, Helper(HelperId), Reject(FallbackReason) }`
  and a generated coverage table mapping every authoritative QuickJS opcode to
  exactly one policy.
- Consumes the dispatch/helper contract from Task 8.

- [ ] **Step 1: Generate a failing opcode coverage test**

```rust
#[test]
fn every_authoritative_opcode_has_an_explicit_policy() {
    for opcode in linked_opcode_table() {
        let policy = tier1_policy(opcode.id).expect("missing opcode policy");
        assert!(matches!(policy, Tier1Policy::Native | Tier1Policy::Helper(_) | Tier1Policy::Reject(_)));
    }
}

#[test]
fn ordinary_sync_programs_are_baseline_eligible() {
    for script in include_str!("fixtures/tier1-programs.txt").split("\n---\n") {
        let run = forced_baseline(script);
        run.assert_same_as_interpreter();
        assert_eq!(run.metrics().compile_rejects, 0);
    }
}
```

- [ ] **Step 2: Run coverage and observe missing policies**

Run: `cargo test -p rquickjs-jit --test opcodes --test differential`

Expected: FAIL listing every opcode without an explicit policy.

`jit/build.rs` reads the generated sys opcode metadata artifact, emits only
names/IDs/formats to `OUT_DIR/opcodes.rs`, and fails if its fingerprint differs
from the linked runtime. `policy.rs` assigns the closed policy enum. The test
compares authoritative count and fingerprint, so a QuickJS opcode update cannot
compile with an incomplete policy table.

- [ ] **Step 3: Cover stack, locals, globals, closure, and control flow**

Implement native or helper lowering for compact local/arg/ref variants,
uninitialized checks, closure references, globals, catch/gosub/ret/finally,
switches, `typeof`, logical operations, and iterator control. Preserve bytecode
PC at every possible throw.

- [ ] **Step 4: Cover objects, arrays, strings, calls, and conversions**

Use helpers for fields, array elements, private fields, class/closure creation,
constructors, methods, spread/apply, iterators, templates, regexp, BigInt,
strings, symbols, conversions, delete, `in`, and `instanceof`. Directly lower
only operations whose ownership and side-effect order are proven.

For every `Native` or `Helper` opcode, `opcode-cases.json` contains at least one
normal result, one applicable numeric/tag edge, one applicable exception or
non-throw assertion, one ownership/GC case, and one coercion-order case when the
opcode can invoke user code. The coverage test fails when a required dimension
is absent; it then executes each case in interpreter and forced Tier 1 modes.

- [ ] **Step 5: Categorically reject high-risk function kinds**

Reject generator/async/async-generator functions, direct eval, `with` scope
opcodes, and unsupported exception shapes with stable `FallbackReason` values.
The interpreter must execute them normally. No `Reject` variant may use an
uncategorized string.

- [ ] **Step 6: Run differential fixture matrix**

Run: `cargo test -p rquickjs-jit --test opcodes --test differential --release`

Expected: PASS; coverage output contains no missing policy. Forced baseline
must enter native code for every eligible fixture, while rejected fixtures
must report the expected fallback category.

- [ ] **Step 7: Commit Tier 1 coverage**

```bash
git add jit/src sys/quickjs/quickjs.c jit/tests/opcodes.rs jit/tests/differential.rs
git commit -m "feat(jit): cover synchronous QuickJS bytecode in Tier 1"
```

### Task 10: Background compilation, installation, invalidation, and hot reload

**Files:**
- Modify: `jit/src/runtime/coordinator.rs`
- Modify: `jit/src/runtime/install.rs`
- Modify: `jit/src/runtime/invalidate.rs`
- Modify: `jit/src/code_cache/mod.rs`
- Modify: `jit/src/code_cache/artifact.rs`
- Modify: `jit/src/code_cache/evict.rs`
- Modify: `sys/quickjs/quickjs.c`
- Create: `jit/tests/background.rs`
- Modify: `jit/tests/lifecycle.rs`
- Modify: `jit/tests/support/mod.rs`

**Interfaces:**
- Produces bounded worker pool, runtime-thread completion drain, generation invalidation, lazy unlink, and clean shutdown.
- Consumes `Compiler`, snapshots, and executable artifacts from Tasks 3-8.

- [ ] **Step 1: Write nonblocking and hot-reload tests**

```rust
#[test]
fn foreground_never_waits_for_compiler() {
    let h = Harness::compiler_blocked();
    let started = Instant::now();
    assert_eq!(h.call("hot"), 42);
    assert!(started.elapsed() < Duration::from_millis(5));
    assert_eq!(h.metrics().interpreted_while_compiling, 1);
}

#[test]
fn reloaded_function_discards_old_completion() {
    let h = Harness::compiler_blocked();
    h.load("export function f(){return 1}");
    h.make_hot("f");
    h.reload("export function f(){return 2}");
    h.release_compiler();
    assert_eq!(h.call("f"), 2);
    assert_eq!(h.metrics().stale_results, 1);
}
```

- [ ] **Step 2: Run tests and demonstrate synchronous behavior failure**

Run: `cargo test -p rquickjs-jit --test background --test lifecycle`

Expected: FAIL because compilation is not yet queued off-thread.

- [ ] **Step 3: Implement bounded background workers**

Copy only `CompileSnapshot` and target/config into jobs. Workers return
artifact or categorized failure through a bounded completion queue. Catch worker
panics and turn them into `CompilerPanicked`; do not unwind through C.

Enforce separate limits for pending jobs, snapshot bytes, estimated compiler
IR bytes, wall-clock compile time, artifact metadata bytes, generated code, and
attempt count. Check the time budget at lowering pass boundaries and discard a
late Cranelift result. Inject each exhaustion independently and assert the
runtime disables only the affected request/tier while interpretation continues.

Add `Harness::compiler_blocked`, `call`, `load`, `make_hot`, `reload`, and
`release_compiler` using barriers rather than sleeps. The 5 ms foreground smoke
assertion runs only in release without sanitizers; the statistical latency gate
remains in Task 14.

- [ ] **Step 4: Drain and install only on runtime thread**

Call completion drain at function boundaries, pending-job entry, explicit
`Jit::poll`, and OSR polls. Recheck runtime ID, function ID, generation, ABI,
CPU features, state, and code quota before installation.

- [ ] **Step 5: Implement lazy invalidation and shutdown**

Retire functions on QuickJS bytecode finalization. Mark artifacts invalid for
new calls without patching executable memory. On guard/runtime drop, stop
submission, detach QuickJS, drain/cancel jobs, wait for execution pins, and
release artifacts before runtime destruction.

- [ ] **Step 6: Run concurrency stress**

Run: `cargo test -p rquickjs-jit --test background --test lifecycle --release -- --test-threads=8`

Expected: PASS for 1,000 repeated create/hot/reload/drop cycles and multiple
isolated runtimes.

- [ ] **Step 7: Commit background tiering**

```bash
git add jit/src/runtime jit/src/code_cache sys/quickjs/quickjs.c jit/tests/background.rs jit/tests/lifecycle.rs
git commit -m "feat(jit): compile and install code off-thread"
```

### Task 11: Hot-call policy, hot-loop counters, and Tier 1 OSR

**Files:**
- Modify: `sys/quickjs/quickjs-jit.h`
- Modify: `sys/quickjs/quickjs.c`
- Modify: `jit/src/runtime/hotness.rs`
- Create: `jit/src/runtime/osr.rs`
- Modify: `jit/src/ir/frame_state.rs`
- Modify: `jit/src/compiler/baseline.rs`
- Create: `jit/tests/osr.rs`
- Modify: `jit/tests/support/mod.rs`

**Interfaces:**
- Produces call/backedge feedback, `OsrKey { function: FunctionKey, pc: u32 }`,
  `OsrMap { key, entry_offset, stack_depth, live_slots: Box<[SlotKind]> }`, and
  adaptive thresholds. The existing vtable `acquire_entry(..., pc)` returns the
  OSR entry; no second C entry ABI is introduced.
- Consumes frame-compatible Tier 1 artifacts and completion drain.

- [ ] **Step 1: Write hotness and first-invocation OSR tests**

```rust
#[test]
fn long_first_call_enters_at_loop_header() {
    let run = automatic("function f(n){let s=0;for(let i=0;i<n;i++)s+=i;return s} f(5_000_000)");
    assert_eq!(run.value().as_f64(), 12_499_997_500_000.0);
    assert!(run.metrics().osr_entries >= 1);
}

#[test]
fn short_cold_callbacks_never_queue() {
    let run = automatic("function f(x){return x+1}; for(let i=0;i<8;i++)f(i)");
    assert_eq!(run.metrics().compile_queued, 0);
}
```

- [ ] **Step 2: Run OSR test and confirm zero entries**

Run: `cargo test -p rquickjs-jit --test osr`

Expected: first test FAIL because OSR entry count is zero.

- [ ] **Step 3: Add saturating counters with low disabled overhead**

Counters exist only on attached runtimes. Call counters trigger snapshots at
the configured threshold. Backedge counters trigger at verified loop headers.
Use saturating counters and a queued bit to prevent duplicate submissions.

- [ ] **Step 4: Emit and validate OSR entry maps**

At each eligible loop header, store expected PC, stack height, live args/locals,
and tagged slot kinds. Interpreter polling enters only when the installed map
matches the current frame exactly. Native entry begins after the backedge and
preserves interrupt cadence.

- [ ] **Step 5: Tune adaptive thresholds without hard-coding gpui-shell**

Scale thresholds by bytecode size, loop presence, previous compile cost, helper
density, and measured invocation work. Keep the default base values 32 calls
and 56 loop iterations. Record why each compilation was queued.

Add `automatic(source) -> Run` to test support. OSR tests use deterministic
synchronous completion at the next backedge poll; production automatic mode
continues compiling asynchronously.

- [ ] **Step 6: Run OSR, interrupts, and cold-overhead benchmark**

Run: `cargo test -p rquickjs-jit --test osr --test semantics --release`

Run: `cargo bench -p rquickjs-jit --bench tiering -- cold_dispatch`

Expected: tests PASS; disabled/no-vtable call overhead is within measurement
noise and cold attached overhead is recorded for the report.

- [ ] **Step 7: Commit OSR and hotness policy**

```bash
git add sys/quickjs jit/src/runtime jit/src/ir/frame_state.rs jit/src/compiler/baseline.rs jit/tests/osr.rs
git commit -m "feat(jit): tier hot calls and OSR hot loops"
```

### Task 12: Tier 2 feedback, SSA, guards, and exact deoptimization

**Files:**
- Create: `jit/src/runtime/feedback.rs`
- Create: `jit/src/ir/optimized.rs`
- Create: `jit/src/compiler/optimized.rs`
- Modify: `jit/src/ir/frame_state.rs`
- Modify: `jit/src/runtime/coordinator.rs`
- Modify: `sys/quickjs/quickjs.c`
- Create: `jit/tests/deopt.rs`
- Create: `jit/tests/optimized.rs`
- Modify: `jit/tests/support/mod.rs`

**Interfaces:**
- Produces bounded `TypeFeedback`, SSA `OptimizedFunction`, `DependencyKey`,
  `GuardId`, `DeoptMap { guard, resume_pc, side_effect_epoch, slots }`, side-exit
  counters, and Tier 2 artifacts.
- Consumes Tier 1 as interpreter-compatible deopt target and function generations for invalidation.

- [ ] **Step 1: Write optimized and deopt side-effect tests**

```rust
#[test]
fn monomorphic_numeric_loop_stays_unboxed() {
    let run = force_optimized(NUMERIC_SUM);
    assert_eq!(run.metrics().tier2_entries, 1);
    assert!(run.metrics().boxes_elided > 0);
    run.assert_same_as_interpreter();
}

#[test]
fn guard_failure_does_not_repeat_getter_side_effect() {
    let script = r#"
      let calls=0;
      function f(o){ const x=o.value; return x+1 }
      const a={get value(){calls++; return 1}};
      for(let i=0;i<100;i++) f(a);
      const result=f({get value(){calls++; return "x"}});
      [result,calls]
    "#;
    let run = force_optimized(script);
    assert_eq!(run.value_json(), r#"["x1",101]"#);
    assert!(run.metrics().deopts >= 1);
}
```

- [ ] **Step 2: Run tests and verify Tier 2 is absent**

Run: `cargo test -p rquickjs-jit --test optimized --test deopt`

Expected: FAIL for missing feedback/optimized compiler.

- [ ] **Step 3: Collect bounded runtime feedback**

Record tagged numeric kinds, monomorphic call target, property shape/slot, and
exit counts in fixed-size feedback entries keyed by function generation and
bytecode PC. Megamorphic or unstable sites transition irreversibly to generic
for that generation after the configured diversity limit.

- [ ] **Step 4: Build QuickJS-specific CFG/SSA and optimization passes**

Perform liveness prepass, abstract frame interpretation, phi insertion,
representation selection, constant folding, redundant guard elimination,
local CSE/DCE, loop-invariant motion, and small monomorphic inlining. Every
optimization must preserve JavaScript `NaN`, negative zero, integer overflow,
coercion, getter/proxy order, and exception behavior.

- [ ] **Step 5: Emit complete frame-state metadata and deoptimizer**

Each guard maps machine registers/spills/constants/materializations to the exact
interpreter PC and slot set. Deopt first pins Tier 1 or interpreter target,
materializes live values, restores ownership counts, then resumes after already
completed side effects. Validate the map before publishing code.

Require a complete live-slot/materialization map at every guard and every
GC-capable/reentrant helper. Before such a helper, materialize each owning
`JSValue` into C-visible frame storage; unboxed scalars need no GC root but must
have deopt materialization recipes. Stress mode forces allocation, cycle GC,
and a reentrant JS call at each location.

- [ ] **Step 6: Add hot-exit policy and lazy unlinking**

Count exits. Compile a side path only for a stable exit after 10 hits; demote
unstable optimized code and apply exponential retry backoff. Generation
invalidation marks dependency records and prevents future Tier 2 entry without
patching executable pages.

Add `force_optimized(source) -> Run` to test support. It performs bounded
warmup until Tier 2 is installed, resets measurement counters only, and then
evaluates the asserted expression without clearing feedback or code.

- [ ] **Step 7: Run optimized differential matrix**

Run: `cargo test -p rquickjs-jit --test optimized --test deopt --test differential --release`

Expected: PASS for numeric, shape change, global mutation, exception, inlining,
allocation, OSR-to-Tier-2, and hot-reload cases.

- [ ] **Step 8: Commit Tier 2**

```bash
git add jit/src/runtime/feedback.rs jit/src/ir jit/src/compiler/optimized.rs sys/quickjs/quickjs.c jit/tests/deopt.rs jit/tests/optimized.rs
git commit -m "feat(jit): optimize hot QuickJS code with exact deopt"
```

### Task 13: Full correctness corpus, fuzzing, and advertised coverage gate

**Files:**
- Modify: `jit/tests/differential.rs`
- Create: `jit/tests/test262.rs`
- Create: `jit/tests/quickjs_suite.rs`
- Create: `jit/tests/regressions/mod.rs`
- Create: `jit/src/bin/jit-test262.rs`
- Create: `jit/fuzz/Cargo.toml`
- Create: `jit/fuzz/fuzz_targets/snapshot.rs`
- Create: `jit/fuzz/fuzz_targets/verifier.rs`
- Create: `jit/fuzz/fuzz_targets/differential.rs`
- Create: `jit/fuzz/fuzz_targets/frame_state.rs`
- Create: `jit/fuzz/fuzz_targets/lowering.rs`
- Create: `jit/fuzz/fuzz_targets/relocations.rs`
- Create: `jit/fuzz/corpus/README.md`
- Create: `scripts/run-jit-test262.sh`
- Create: `jit/src/bin/jit-coverage-report.rs`

**Interfaces:**
- Produces reproducible interpreter/Tier1/Tier2 differential runner and machine-readable opcode/feature coverage report.
- Consumes all execution tiers.

- [ ] **Step 1: Add a coverage gate that fails on fallback masking**

```rust
#[test]
fn advertised_tier1_fixtures_execute_native_code() {
    for case in coverage_manifest().tier1_cases() {
        let run = run_case(case, Mode::ForceTier1);
        assert_eq!(run.result, case.expected);
        assert!(run.metrics.tier1_entries > 0, "{} fell back", case.name);
    }
}
```

- [ ] **Step 2: Build Rust-hosted suite runners and record interpreter baselines**

Run: `git -C sys/quickjs submodule update --init test262`

Implement `jit-test262` with `JitRuntime`: parse Test262 YAML metadata, append
harness includes, support script/module modes, async completion, negative
parse/runtime cases, and an explicit checked-in exclusion manifest for
unsupported agent/realm features. The C `run-test262` binary is an interpreter
reference only and does not count as JIT coverage.

Run: `cargo test -p rquickjs-jit --test test262 --release -- --ignored interpreter`

Expected: produce `target/jit-test262/interpreter.json` with pass/fail/skip counts
matching the pinned QuickJS exclusion configuration.

Run: `cargo test -p rquickjs-jit --test quickjs_suite --release -- interpreter`

Expected: run all `sys/quickjs/tests` scripts through the Rust host and write
`target/jit-quickjs/interpreter.json` with explicit exclusion reasons.

- [ ] **Step 3: Run automatic and forced eligible modes**

Run: `cargo test -p rquickjs-jit --test test262 --release -- --ignored automatic`

Run: `cargo test -p rquickjs-jit --test test262 --release -- --ignored force-tier1-eligible`

Run: `cargo test -p rquickjs-jit --test quickjs_suite --release -- automatic`

Expected: no result differs from interpreter baseline; forced eligible mode
also satisfies native-entry coverage assertions.

- [ ] **Step 4: Add structured random-program differential generation**

Generate terminating functions with bounded expressions, branches, loops,
closures, objects, arrays, getters, proxies, conversion hooks, throws, and
finally blocks. Compare value serialization, exception, event trace, and
ownership counters across modes. Seed and minimized source are printed on
failure and saved under `jit/tests/regressions/`.

The canonical observation object includes own property names/descriptors,
prototype-visible values selected by the fixture, getter/proxy/coercion event
order, host events, interrupt result, normalized stable stack frames, and weak
liveness probes after forced cycle collection. Differential equality compares
every populated field, not only the returned JSON value.

Store regressions as JSON containing seed, source, invocation, canonical
value/exception/events, and required tier. `jit/tests/regressions/mod.rs`
replays every file under `cases/`. Minimize with `cargo test -p rquickjs-jit
--test differential -- minimize <seed>`.

- [ ] **Step 5: Add fuzz targets and seed corpus**

Snapshot/verifier targets accept arbitrary bytes and metadata. Differential
target accepts structured programs. Frame-state target mutates guard maps and
must reject invalid maps without executing them. Seed from opcode tests,
QuickJS tests, and every minimized regression.

The lowering target accepts verified functions and must never panic or emit
invalid Cranelift IR. The relocation target mutates offset/type/addend/symbol
combinations and rejects out-of-range or misaligned targets before writing.
Corpora live at `jit/fuzz/corpus/<target>/` with a versioned binary header.

- [ ] **Step 6: Run correctness gate and bounded fuzz smoke**

Run: `cargo test -p rquickjs-jit --all-targets --release`

Run: `cargo +nightly fuzz run --fuzz-dir jit/fuzz verifier -- -max_total_time=60`

Run: `cargo +nightly fuzz run --fuzz-dir jit/fuzz frame_state -- -max_total_time=60`

Run: `cargo +nightly miri test -p rquickjs-jit --lib`

Expected: all tests PASS, both fuzz targets complete without crash, and Miri
finds no UB in pure Rust state machines. Fault injection independently covers
snapshot allocation, worker panic, stale completion, queue saturation,
compiler-memory/time exhaustion, denied executable memory, invalid relocation,
and shutdown races; each leaves interpreter evaluation functional.

- [ ] **Step 7: Commit correctness infrastructure**

```bash
git add jit/tests jit/fuzz scripts/run-jit-test262.sh jit/src/bin/jit-coverage-report.rs
git commit -m "test(jit): gate semantics with Test262 and fuzzing"
```

### Task 14: Reproducible benchmarks and profitability policy

**Files:**
- Create: `jit/benches/micro.rs`
- Create: `jit/benches/algorithms.rs`
- Create: `jit/benches/tiering.rs`
- Create: `benchmarks/Cargo.toml`
- Create: `benchmarks/suites.lock`
- Create: `benchmarks/schema/jit-benchmark-v1.json`
- Create: `benchmarks/scripts/numeric.js`
- Create: `benchmarks/scripts/collections.js`
- Create: `benchmarks/scripts/strings-json.js`
- Create: `benchmarks/scripts/calls-closures.js`
- Create: `benchmarks/scripts/adversarial.js`
- Create: `benchmarks/run.rs`
- Create: `benchmarks/report.rs`
- Modify: `jit/src/runtime/hotness.rs`
- Modify: `jit/src/code_cache/evict.rs`
- Create: `docs/jit-performance.md`

**Interfaces:**
- Produces JSON schema `jit-benchmark-v1` and Markdown report.
- Produces profitability decision `Interpret | Baseline | Optimize` with categorized rationale.
- Consumes runtime metrics and all tiers.

- [ ] **Step 1: Write benchmark-schema and policy tests**

```rust
#[test]
fn unprofitable_host_heavy_function_stays_baseline() {
    let profile = Profile { bytecodes: 100, helper_calls: 90, compile_ns: 80_000, executions: 40 };
    assert_eq!(Profitability::default().decide(profile), Decision::Baseline);
}

#[test]
fn hot_numeric_loop_optimizes() {
    let profile = Profile { bytecodes: 20_000_000, helper_calls: 2, compile_ns: 120_000, executions: 1 };
    assert_eq!(Profitability::default().decide(profile), Decision::Optimize);
}
```

- [ ] **Step 2: Record pre-JIT interpreter baseline**

Create a non-published workspace package whose two binaries are declared
explicitly:

```toml
[package]
name = "rquickjs-jit-benchmarks"
version.workspace = true
edition.workspace = true
publish = false

[[bin]]
name = "jit-bench"
path = "run.rs"

[[bin]]
name = "jit-bench-report"
path = "report.rs"
```

`suites.lock` records repository URL, exact commit, subdirectory, license, and
SHA-256 for each imported QuickJS, SunSpider, and runnable JetStream corpus.
The import command resolves a requested upstream ref once, writes the commit
and hashes, and thereafter refuses network access or a hash mismatch. Never
benchmark a moving branch.

Run: `cargo run --release --manifest-path benchmarks/Cargo.toml -- run --mode interpreter --output target/bench/interpreter.json`

Expected: JSON validates against `jit-benchmark-v1.json` and includes target
triple, machine/OS/kernel/CPU/governor-or-power-mode, Rust and LLVM versions,
QuickJS source revision and opcode fingerprint, executable size, warmup policy,
raw samples, median, MAD, bootstrap 95% confidence interval, P95, P99, peak RSS,
code/compiler memory, and zero JIT-entry counters.

- [ ] **Step 3: Implement benchmark groups and runner**

Measure cold runtime/context creation, first eval, threshold crossing, compile,
OSR, break-even, steady state, code bytes, peak compiler memory, and fallback.
Run QuickJS benchmarks, SunSpider, and runnable JetStream components; list every
excluded test and reason in the generated report.

Use at least 30 measured process samples after five warmups for latency and at
least 10 one-second sampling windows for throughput. Preserve raw nanoseconds
and environment provenance. Compare paired samples on the same host; flag a
regression only when the bootstrap interval excludes zero and the configured
limit is exceeded. Track stripped binary-size deltas separately from runtime
RSS.

- [ ] **Step 4: Implement measured profitability and benefit-aware eviction**

Estimate saved dispatch/helper work from counters, subtract compile/install
cost, and require positive amortized benefit before Tier 2. Feed measured entry
time and code size to eviction. Preserve deterministic fixed-policy mode for
tests and reports.

- [ ] **Step 5: Run full interpreter/Tier1/Tier2/automatic comparison**

Run: `cargo run --release --manifest-path benchmarks/Cargo.toml -- compare --modes interpreter,tier1,tier2,automatic --output target/bench/comparison.json --report docs/jit-performance.md`

Expected: report includes every workload, not only improvements, and computes
geometric means plus 10x representative-kernel and regression gates. The
command exits nonzero if compute geometric mean is below 5x, every designated
representative kernel is below 10x, gpui-shell steady state is below 2x, or a
startup/hot-reload/P99 regression is both statistically significant and above
5%. A failed target is reported as evidence, never hidden by retuning or
fallback.

- [ ] **Step 6: Commit benchmark and policy work**

```bash
git add jit/benches jit/src/runtime/hotness.rs jit/src/code_cache/evict.rs benchmarks docs/jit-performance.md
git commit -m "perf(jit): benchmark and tune tier profitability"
```

### Task 15: gpui-shell validation, CI matrix, documentation, and release audit

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`
- Modify: `core/README.md`
- Modify: `sys/README.md`
- Create: `jit/README.md`
- Create: `docs/jit-upgrades.md`
- Create: `scripts/bench-gpui-shell.sh`
- Create: `scripts/check-jit-release.sh`
- External integration during validation: `../gpui-component/crates/shell/Cargo.toml`
- External integration during validation: `../gpui-component/crates/shell/src/engine/quickjs/mod.rs`
- External benchmark extension during validation: `../gpui-component/crates/shell/src/tests/benchmark.rs`

**Interfaces:**
- Produces documented setup for native and WASM consumers, macOS entitlement guidance, upgrade workflow, gpui-shell comparison, and complete release evidence.
- Consumes all previous tasks.

- [ ] **Step 1: Write the gpui-shell integration patch under authorized scope**

Change the shell dependency to include `rquickjs-jit`, store `JitRuntime` instead
of `JsRuntime`, and keep calls working through deref. Add a shell feature
`quickjs-jit` enabled on native desktop defaults and excluded for WASM. On
missing macOS entitlement, log one diagnostic and continue interpreted.

The integration test must assert:

```rust
#[gpui::test]
fn jit_does_not_change_snapshot_or_render_count(cx: &mut TestAppContext) {
    let interpreted = render_fixture(cx, EngineMode::Interpreter);
    let automatic = render_fixture(cx, EngineMode::AutomaticJit);
    assert_eq!(automatic.snapshot, interpreted.snapshot);
    assert_eq!(automatic.script_renders, interpreted.script_renders);
}
```

- [ ] **Step 2: Extend shell benchmarks without replacing existing metrics**

Measure existing realistic panel, recorded-call stages, event handlers, state
transforms, async continuation, first window, and hot reload in interpreter and
automatic modes. Report total script render plus native-host time so unchanged
host cost remains visible.

- [ ] **Step 3: Run gpui-shell validation against local rquickjs**

Run: `scripts/bench-gpui-shell.sh ../gpui-component target/gpui-shell-jit-report.json`

Expected: all shell tests PASS; snapshots/render counts match; report contains
steady-state speedup, first-window delta, hot-reload delta, P99, native hit rate,
and fallback categories. If the sibling repository is outside the writable
scope, stop before editing it and request that exact scope; do not copy the
shell into this repository.

- [ ] **Step 4: Add CI platform and regression jobs**

Add native JIT tests on Linux x86-64/AArch64, macOS x86-64/AArch64, and Windows
x86-64/AArch64. Use native hosted runners for execution: `ubuntu-24.04`,
`ubuntu-24.04-arm`, `macos-15-intel`, `macos-15`, `windows-2025`, and a pinned
Windows 11 ARM64 runner label documented in the workflow. If the repository
does not have the ARM64 Windows runner, keep that job required-but-manually
provisioned rather than substituting cross-compilation for execution. Keep
non-JIT tests on all current targets and add a WASM check that asserts native
JIT dependencies are absent. Run Test262 and long fuzzing on scheduled CI; run
bounded semantics/platform tests on pull requests.

- [ ] **Step 5: Document API, entitlements, diagnostics, and upgrades**

`jit/README.md` includes minimal builder use, attach guard use, config, metrics,
fallback behavior, supported matrix, and macOS signing example.
`docs/jit-upgrades.md` requires ABI fingerprint review, opcode regeneration,
differential/Test262, platform tests, performance comparison, and coverage
publication for each QuickJS update.

Document the exact update command sequence: update `sys/quickjs`, regenerate all
nine checked-in bindings and opcode metadata, update the ABI source revision and
opcode fingerprint, run snapshot golden tests, run interpreter/automatic/forced
eligible Test262 comparisons, run six native platform jobs, then publish the
coverage and performance diff. An ABI major mismatch hard-disables attachment;
an opcode fingerprint mismatch rejects snapshots and falls back per function.

- [ ] **Step 6: Run the complete release audit**

Run: `scripts/check-jit-release.sh`

The script runs formatting, clippy, workspace tests, non-JIT tests, JIT tests,
WASM check, opcode coverage, Test262 comparison, sanitizer subset, benchmark
schema validation, and documentation links. It fails if evidence files are
missing or if fallback masks advertised coverage.

- [ ] **Step 7: Review requirements against authoritative evidence**

Verify and record:

```text
semantic equality       -> QuickJS + Test262 + differential reports
independent crate       -> cargo dependency graph and source ownership
desktop support         -> six native CI jobs
WASM disabled           -> wasm build graph and interpreted smoke test
W^X                     -> platform tests and OS protection queries
background compilation  -> foreground nonblocking integration test
hot reload safety       -> generation/stale-result stress test
5x/10x compute targets  -> complete benchmark report
2x gpui-shell target    -> shell workload report
startup/P99 <= 5% loss  -> cold and latency report
```

Any item without direct evidence remains incomplete.

- [ ] **Step 8: Commit integration documentation and CI**

```bash
git add .github/workflows/ci.yml README.md core/README.md sys/README.md jit/README.md docs/jit-upgrades.md scripts/bench-gpui-shell.sh scripts/check-jit-release.sh
git commit -m "docs(jit): validate platforms and gpui-shell integration"
```

## Final verification

- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run `cargo clippy --workspace --all-targets --features full-async,jit-abi,bindgen -- -D warnings`.
- [ ] Run `cargo test --workspace --all-targets --features full-async,jit-abi,bindgen` and `cargo test -p rquickjs-jit --all-targets --features compiler`.
- [ ] Run `cargo test --workspace --all-targets --no-default-features --features full-async,bindgen` to prove the interpreter-only configuration.
- [ ] Run `cargo check -p rquickjs-jit --target wasm32-wasip1` and inspect `cargo tree` to prove no native backend is linked.
- [ ] Run the QuickJS suite, Test262 comparison, sanitizer subset, fuzz smoke, complete core benchmark, and gpui-shell benchmark commands defined above.
- [ ] Inspect `git diff --check`, `git status --short`, generated performance reports, coverage report, and six native CI results.
- [ ] Perform the requirement-to-evidence audit from Task 15 before claiming completion.
