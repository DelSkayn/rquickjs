# QuickJS Tiered JIT Design

**Status:** Approved design  
**Date:** 2026-08-29  
**Repository:** `rquickjs`  
**Primary consumer:** `gpui-shell`

## 1. Purpose

Add an optional, production-quality native JIT to QuickJS while preserving
JavaScript behavior exactly. The implementation must be useful to embedders in
general and must specifically improve `gpui-shell`, where QuickJS is the
application runtime for GPUI desktop applications.

The implementation has four non-negotiable properties:

1. JavaScript semantics remain correct. A function that cannot be compiled
   safely continues in the interpreter without observable behavioral changes.
2. Most implementation lives in an independent `rquickjs-jit` crate. Updating
   QuickJS or rquickjs should require adapting a small, versioned integration
   layer rather than porting a JIT embedded throughout `quickjs.c`.
3. Native JIT execution supports macOS, Windows, and Linux on x86-64 and
   AArch64. WebAssembly builds do not enable the JIT and retain the interpreter.
4. Performance claims are workload-specific and reproducible. Compute-heavy
   JavaScript should approach or exceed a 10x speedup on representative cases;
   real `gpui-shell` steady-state workloads target at least 2x without harming
   startup, hot reload, or tail latency.

## 2. Context and constraints

### 2.1 rquickjs and QuickJS

`rquickjs-sys` builds quickjs-ng from the `sys/quickjs` submodule. QuickJS
represents executable JavaScript with the private `JSFunctionBytecode`
structure. Its bytecode, constants, local-variable metadata, closure variables,
stack size, exception handling, generators, stack traces, and reference-count
operations are implemented inside `quickjs.c`.

The public QuickJS API can compile, serialize, deserialize, and call functions,
but it cannot inspect a bytecode function safely or replace the execution of an
individual function. A completely external crate cannot therefore implement a
transparent and correct JIT using only the existing public API.

### 2.2 gpui-shell workload

`gpui-shell` runs QuickJS in the host process on GPUI's foreground thread. A
JavaScript `View` builds a retained Rust snapshot only when invalidated; clean
frames materialize that snapshot without entering JavaScript. Important JIT
workloads are therefore:

- first render and hot reload, where compilation must not add noticeable
  foreground latency;
- repeated view renders after state invalidation;
- event handlers and asynchronous continuations;
- JavaScript business logic and data transformation;
- fine-grained calls from JavaScript builder chains into Rust host functions.

The current shell measurements report roughly 240-340 ns per recorded builder
operation and distinguish script rendering from snapshot materialization.
Machine code can remove interpreter work, but it cannot remove Rust host calls,
argument conversion, arena writes, layout, or paint. A blanket 10x application
speedup is therefore neither an honest nor a useful acceptance criterion.

### 2.3 Reference implementation

`../navi-script/crates/vm/src/jit` demonstrates a useful basic pattern:

- identify bytecode basic blocks;
- lower stack bytecode to Cranelift IR;
- call stable `extern "C"` helpers for complex runtime operations;
- retain a JIT module with the compiled function;
- use an explicit execution state and return codes at the trampoline boundary.

QuickJS requires a deeper integration than that VM. It has dynamic object
semantics, reference counting, closures, exceptions, generators, async state,
interrupts, stack traces, and bytecode structures that are private to the C
engine. This design borrows the compiler/helper separation but does not copy
the execution-state model or assume that every value fits in an `f64` slot.

## 3. Approaches considered

### 3.1 Read private QuickJS layouts externally

An external crate could duplicate `JSFunctionBytecode`, `JSObject`, and stack
frame definitions and cast pointers returned by public APIs.

This is rejected. The layouts are not a public ABI, depend on build flags and
the exact QuickJS revision, and would permit silent memory corruption on an
upgrade. It also cannot safely participate in exception unwinding or object
lifetime management.

### 3.2 Bundle a modified QuickJS in `rquickjs-jit`

An independent crate could build its own modified QuickJS and reproduce the
rquickjs wrapper API.

This is rejected. It creates two QuickJS copies, two FFI binding sets, and
incompatible `Runtime`, `Context`, and `Value` types. It appears independent at
the package boundary while maximizing upgrade and linking cost.

### 3.3 Minimal engine ABI plus an independent JIT crate

This is the selected approach. QuickJS receives a small, versioned integration
ABI and a function-entry dispatch point. `rquickjs-core` receives only the safe
lifecycle bridge needed to attach the backend. Compilation, profiling policy,
code memory, caching, metrics, and almost all tests live in `rquickjs-jit`.

## 4. Crate and source architecture

The workspace adds one independent crate:

```text
jit/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── bytecode/
│   │   ├── decode.rs
│   │   ├── cfg.rs
│   │   ├── stack.rs
│   │   └── verify.rs
│   ├── ir/
│   │   ├── baseline.rs
│   │   ├── optimized.rs
│   │   ├── frame_state.rs
│   │   └── types.rs
│   ├── compiler/
│   │   ├── mod.rs
│   │   ├── baseline.rs
│   │   ├── optimized.rs
│   │   └── helpers.rs
│   ├── runtime/
│   │   ├── coordinator.rs
│   │   ├── hotness.rs
│   │   ├── feedback.rs
│   │   ├── install.rs
│   │   ├── invalidate.rs
│   │   └── osr.rs
│   ├── code_cache/
│   ├── platform/
│   │   ├── linux.rs
│   │   ├── macos.rs
│   │   └── windows.rs
│   ├── config.rs
│   ├── error.rs
│   └── metrics.rs
└── tests/
    ├── differential/
    ├── semantics/
    ├── lifecycle/
    └── benchmarks/
```

The existing repository changes are limited to:

- `sys/quickjs/quickjs-jit.h`: the internal, versioned C ABI;
- narrow additions to `sys/quickjs/quickjs.c` for snapshots, dispatch, safe
  exits, feedback, and lifetime notifications;
- `sys/build.rs` and bindings for feature-gated compilation;
- a feature-gated rquickjs-core registration and lifetime bridge;
- workspace manifests and public feature forwarding.

Neither QuickJS nor rquickjs-core depends on Cranelift or knows about
`gpui-shell`.

## 5. Versioned QuickJS JIT ABI

### 5.1 Compatibility

The ABI exposes:

- an ABI major and minor version;
- a QuickJS source revision fingerprint;
- value-layout, pointer-width, endianness, and build-feature flags;
- structure sizes and offsets only for explicit ABI structures defined in
  `quickjs-jit.h`.

Attachment fails before executing machine code when the ABI is incompatible.
The JIT never reads a private QuickJS structure directly.

### 5.2 Runtime vtable

An opt-in runtime stores a backend vtable and opaque backend pointer. The vtable
supports:

- recording hot calls and loop backedges;
- submitting an owned bytecode snapshot;
- checking and entering installed code;
- retiring a function generation;
- releasing runtime-owned backend state;
- reporting backend memory to QuickJS memory statistics.

With no vtable installed, function entry performs one predictable null check
and follows the current interpreter path.

### 5.3 Owned function snapshot

QuickJS creates an immutable snapshot that remains valid off-thread. It contains
only serialized or copied data:

- bytecode bytes and opcode-table revision;
- argument, local, stack, closure, and function-kind metadata;
- constant descriptors that are safe to copy;
- stable handles for constants that must remain in the runtime;
- exception-region and source-location metadata;
- function identity and monotonically increasing generation.

The background compiler never dereferences a `JSRuntime`, `JSContext`,
`JSObject`, atom, string, or GC pointer. Runtime-owned constants are accessed
through stable handles only after execution returns to the runtime thread.

## 6. Execution tiers

```text
Tier 0: QuickJS interpreter
    │ hot call or hot loop
    ▼
Tier 1: baseline native code
    │ higher heat plus stable feedback
    ▼
Tier 2: optimized SSA native code
```

### 6.1 Tier 0

The existing interpreter remains the semantic oracle and permanent fallback.
It gathers bounded call, loop, type, and exit feedback only for runtimes with a
JIT backend attached.

### 6.2 Tier 1 baseline JIT

Tier 1 prioritizes low compilation latency and easy correctness auditing:

- it uses a frame layout compatible with the interpreter's logical arguments,
  locals, operand stack, bytecode position, and exception state;
- it predecodes operands and control flow, eliminating interpreter dispatch;
- local numeric operations use checked tagged-integer and floating-point fast
  paths;
- complex conversion, object, allocation, property, call, and language
  semantics use stable QuickJS helpers;
- failed type checks branch to generic helpers in the same compiled function;
- no speculative assumption may require arbitrary mid-instruction deopt;
- unsupported bytecode rejects the entire function before installation.

Tier 1 supports on-stack replacement at verified loop headers. A long-running
interpreted loop can enter native code after background compilation completes,
without waiting for a second function call. OSR transfers only at a stack map
whose interpreter and native frame state are proven equivalent.

### 6.3 Tier 2 optimizing JIT

Tier 2 uses a compact CFG and SSA IR with QuickJS-specific operations. Planned
optimizations are:

- representation selection and numeric unboxing;
- constant propagation and folding with JavaScript-correct edge cases;
- redundant tag- and shape-check elimination;
- local common subexpression and dead-code elimination;
- loop-invariant code motion when side effects permit it;
- bounds-check elimination under explicit guards;
- small monomorphic function inlining;
- allocation and boxing elimination only when escape analysis proves safety.

Every speculative node carries or refers to frame-state metadata mapping SSA
values and machine locations back to interpreter arguments, locals, operand
stack, bytecode position, and exception state. A failed guard reconstructs the
exact interpreter state after all preceding side effects and before any later
side effect. Frequently taken exits may receive compiled side paths; unstable
sites are demoted and protected by retry limits.

## 7. Hotness, compilation, and installation

The runtime tracks function entries and loop backedges. Thresholds scale with
bytecode size and observed helper-call density. Small repeated UI callbacks may
tier up through call counts; a single compute-heavy invocation may tier up
through loop counts.

Compilation follows this state machine:

```text
interpreting → queued → compiling → ready → installed
      │           │          │         │         │
      └──────── failure / stale / unprofitable ──┘
```

- Snapshot creation occurs on the runtime thread.
- Cranelift lowering, optimization, code generation, and relocation planning
  occur on bounded background workers.
- Installation occurs on the runtime thread at a function boundary, event-loop
  entry, or verified OSR safe point.
- Queue length, compile memory, code memory, and compile attempts are bounded.
- Repeated failures and type instability use exponential backoff and eventually
  blacklist the affected tier for that function generation.
- A profitability model compares estimated saved interpreter work with measured
  compilation and machine-code cost. Code that cannot amortize itself remains
  interpreted or at Tier 1.

The GPUI foreground thread never waits for a background compilation. A testing
mode permits deterministic synchronous compilation.

## 8. Calls, exceptions, interrupts, and garbage collection

### 8.1 Calls and helpers

Compiled code calls a small C ABI, not private QuickJS functions. Each helper:

- receives an explicit execution/frame handle;
- validates the runtime and function generation in debug builds;
- owns precise input/output value reference-count contracts;
- returns a value or a typed control status;
- may allocate or trigger GC only when its contract says so.

The helper table is generated or declared once so its signatures are shared by
C, Rust bindings, and compiler declarations.

### 8.2 Exceptions

Tier 1 helpers return an exception status and leave the QuickJS exception in
the normal runtime slot. Native code records the current bytecode position and
enters the existing QuickJS exception search and cleanup path. Catch and
finally behavior, stack traces, and uncatchable errors remain owned by QuickJS.

Tier 2 guard deoptimization and language exceptions are distinct. A guard exit
restores an ordinary interpreter frame without creating a JavaScript exception.

### 8.3 Interrupts and safe points

Compiled code polls the existing QuickJS interrupt handler at:

- function entry;
- loop backedges;
- calls and allocation helpers;
- bounded intervals in long straight-line code.

This preserves `gpui-shell` runaway-script enforcement. Interrupts use the
same uncatchable-error behavior as the interpreter.

### 8.4 GC and reference counting

Stack maps identify every live tagged `JSValue` at helpers, safe points, OSR,
and deopt locations. Tier 1 keeps values in interpreter-compatible slots across
operations that may allocate. Tier 2 may keep unboxed scalars in registers but
must materialize every live GC-visible value before a GC-capable helper.

Value ownership is tested as an independent invariant. A compiled opcode must
perform exactly the same `dup`, move, and free operations as the interpreter,
including exceptional exits.

## 9. Invalidation, hot reload, and code ownership

Every bytecode function has a stable identity and generation. Compiled artifacts
record both plus the ABI fingerprint and optimization tier.

When a function or module is replaced:

1. QuickJS increments or retires the generation.
2. New entries stop selecting old code immediately.
3. Pending compilation results for the old generation are discarded.
4. Existing activations may finish using an execution reference.
5. Code is reclaimed after the last execution reference and metadata reference
   disappear.

This is analogous to a world-version boundary: background work can never attach
to a newly loaded function merely because memory addresses were reused. Lazy
unlinking avoids patching executable pages during invalidation.

## 10. Public Rust API

The safe entry point is an owning wrapper:

```rust
use rquickjs_jit::{JitConfig, JitRuntime};

let runtime = JitRuntime::builder()
    .config(JitConfig::default())
    .build()?;
let context = rquickjs::Context::full(&runtime)?;
```

`JitRuntime` owns the ordinary `rquickjs::Runtime` and JIT coordinator and
dereferences to `Runtime`. Its destructor detaches the vtable, stops submissions,
waits for active entries, drops native code and backend state, and only then
allows QuickJS to be destroyed.

An advanced `Jit::attach(&Runtime, JitConfig)` API returns a required guard.
Dropping the guard safely disables new JIT entries. The owning wrapper is the
recommended API because it makes lifetime order structural.

`JitConfig` controls:

- enablement and tier policy;
- call and loop thresholds;
- worker count and queue limits;
- per-runtime code-memory limit;
- optimization level and retry limits;
- synchronous testing mode;
- diagnostics and metrics callbacks.

Runtime metrics expose tier counts, compile time, installation time, code size,
dynamic JIT hit rate, OSR/deopt/side-exit counts, interpreter time estimate,
queue pressure, eviction, and categorized fallback reasons.

Backend failures do not become JavaScript exceptions. Unsupported platforms,
missing execution permission, compiler failures, queue pressure, and
unprofitable code preserve interpretation and produce structured diagnostics.

## 11. Executable memory and platform support

The JIT uses Cranelift code generation and relocation data with its own
`CodeAllocator`; it does not rely on `cranelift-jit::JITModule` for executable
memory. This gives the embedding explicit W^X behavior and handles platform
requirements consistently.

### 11.1 Linux

- Allocate anonymous, non-executable writable pages.
- Write code and apply relocations.
- Flush instruction cache where the architecture requires it.
- Change completed pages to read/execute.
- Never map pages writable and executable simultaneously.

### 11.2 Windows

- Allocate writable pages with `VirtualAlloc`.
- Finalize them with `VirtualProtect` to executable/read-only protection.
- Call `FlushInstructionCache`.
- Register or preserve valid indirect targets according to Control Flow Guard
  policy.
- Release regions with `VirtualFree` after epoch/reference safety permits it.

### 11.3 macOS

- Allocate executable-code storage with `MAP_JIT`.
- Use Apple's per-thread JIT write-protection API or write callback while
  modifying code.
- Flush/synchronize instruction cache before execution.
- Require the `com.apple.security.cs.allow-jit` entitlement for a hardened
  application.
- Use a process-global physical code heap with logical per-runtime ownership
  and quotas because hardened macOS limits the process to one `MAP_JIT` region.

If entitlement or system policy rejects JIT memory, attachment reports one
structured diagnostic and the application remains interpreted.

### 11.4 WebAssembly and other targets

The `rquickjs-jit` public API can be conditionally present for dependency
uniformity, but native compilation is disabled and returns
`UnsupportedPlatform`. Normal rquickjs builds have no JIT dependency and no
behavior change.

## 12. Code cache

The cache is partitioned logically by runtime and indexed by function identity,
generation, tier, target ISA, CPU features, and ABI fingerprint. It tracks code,
relocations, unwind/stack metadata, frame states, and runtime dependencies as
one artifact.

Eviction uses recency plus measured execution benefit, never raw age alone.
Pinned active code cannot be evicted. Tier 2 may replace Tier 1 for future
entries while retaining Tier 1 as a deopt target until dependencies permit its
release.

The initial release uses an in-memory cache only. A persistent native cache is
excluded because secure validation, address-independent relocation, CPU-feature
compatibility, and signed application distribution form a separate design.

## 13. Correctness strategy

### 13.1 Static verifier

Before lowering, the verifier proves:

- complete, non-overlapping opcode decoding;
- valid branch targets and basic-block boundaries;
- consistent stack height and compatible stack kinds at every merge;
- valid constant, local, argument, closure, atom, and exception indices;
- supported function kind and control-flow regions;
- well-formed OSR and deopt points;
- bounded compiler resource use.

Unproven input is rejected, not guessed.

### 13.2 Test layers

1. Unit tests cover decode, CFG, stack analysis, verification, IR, frame states,
   hotness, queues, cache, platform allocation, and state machines.
2. Opcode tests cover normal results, edge values, exceptions, ownership, and
   observable coercion ordering for every claimed opcode.
3. Differential tests run scripts under forced interpreter, automatic tiering,
   forced Tier 1, and eligible Tier 2 modes.
4. Engine tests run QuickJS's suite and the applicable complete Test262 corpus.
5. Platform and integration tests cover the six OS/architecture combinations,
   multiple runtimes, low memory, teardown, hot reload, OSR, deopt, and
   `gpui-shell`.

Differential comparison includes return values, exception type and message,
property descriptors, getter/proxy/coercion call order, host-event sequences,
interrupt behavior, stack traces where stable, and post-GC memory/liveness.
Generated programs are minimized on failure and retained as regressions.

Automatic fallback cannot hide missing advertised coverage. Tests assert both
the result and the expected tier/exit metrics.

### 13.3 Tooling

- C and Rust sanitizer builds exercise interpreter/JIT transitions.
- Miri covers pure Rust state machines and ownership components that do not
  execute native code.
- Fuzz targets cover bytecode snapshots, verifier, lowering, relocation input,
  frame-state reconstruction, and differential execution.
- Fault injection covers allocation failure, worker panic, stale compilation,
  queue saturation, denied executable memory, and runtime shutdown races.

## 14. Performance evaluation

### 14.1 Measurement rules

Every result records source revision, target, OS, CPU, compiler, power mode,
configuration, warmup, raw samples, and JIT metrics. Reports distinguish:

- cold startup;
- first execution;
- threshold crossing and compile time;
- OSR time;
- amortization/break-even point;
- warmed steady state.

Use repeated samples and report median, P95/P99, dispersion or confidence
intervals, and memory. Do not report only the fastest batch or only workloads
that improve.

### 14.2 Benchmark groups

- Opcode microbenchmarks isolate dispatch, locals, branches, calls, numeric
  fast paths, property operations, exceptions, and helper overhead.
- Compute workloads cover numeric kernels, algorithms, parsing, collections,
  strings, JSON, closures, and call-heavy code.
- Established suites include QuickJS benchmarks, SunSpider, and runnable
  JetStream components, with every exclusion listed and justified.
- Adversarial workloads cover cold code, type instability, megamorphic access,
  exception-heavy paths, compile storms, and code-cache pressure.
- `gpui-shell` workloads cover realistic panel description, builder-boundary
  stages, event handlers, state transformation, async continuations, module
  loading, first window, and hot reload.
- Non-JIT frame costs such as snapshot materialization, layout, and paint remain
  in the report to show the end-to-end ceiling rather than attributing them to
  the JIT.

### 14.3 Acceptance gates

- Interpreter-only behavior and performance remain available and regression
  tested.
- Automatic JIT passes the same semantic suites.
- Compute-heavy benchmarks target a geometric-mean speedup of at least 5x,
  with representative hot kernels targeting at least 10x.
- Real steady-state `gpui-shell` workloads target at least 2x where script time
  is material.
- First-window, hot-reload, and P99 script-render latency must not regress by
  more than 5% in the default automatic policy.
- An individual workload whose total time regresses significantly must remain
  interpreted, use a lower tier, or cause the profitability policy to change.
- Binary size, peak compilation memory, native-code memory, coverage, dynamic
  hit rate, and every fallback category are release-report requirements.

A missed performance target is reported honestly and investigated. It is never
resolved by weakening semantics, excluding unfavorable results silently, or
allowing fallback to masquerade as JIT coverage.

## 15. Lessons adopted from other JITs

### 15.1 V8 Sparkplug

Adopt the baseline principle: compile bytecode quickly, preserve an
interpreter-compatible logical frame, and call shared builtins for complex
semantics. This reduces dispatch without duplicating the full language runtime
and makes stack walking, exceptions, OSR, and later deoptimization tractable.

### 15.2 V8 Maglev

Adopt a compact CFG/SSA optimizing tier, prepass liveness, representation
selection, runtime feedback, explicit dependency tracking, and frame-state
metadata. Avoid a large general-purpose graph optimizer in the first version.

### 15.3 LuaJIT

Adopt separate hot-call, hot-loop, and hot-exit counters; bounded code caches;
retry limits; inspection metrics; and the principle that every speculative exit
needs a complete restoration snapshot. Do not adopt full trace recording as the
initial architecture because QuickJS's C interpreter and language state make a
correct meta/trace integration substantially more invasive.

### 15.4 HotSpot

Adopt OSR so a rare function with a long-running loop benefits within its first
invocation, and make safe points an explicit runtime contract rather than an
incidental helper side effect.

### 15.5 Julia

Adopt generation/world-version-style validation for background compilation and
hot reload. No compiled result may install into a different definition merely
because an address or function slot was reused.

### 15.6 Luau

Adopt profitability controls, native-code limits, per-function diagnostics, and
workload-specific expectations. Native code is most valuable for computation
and may provide limited gains when execution is dominated by host API calls.

### 15.7 PyPy

Adopt differential testing and the insight that allocation/boxing removal can
produce large gains. Do not adopt meta-tracing: QuickJS is not written in a
JIT-generator language, and retrofitting a tracer around its C interpreter
would work against the minimal-upstream-adapter requirement.

## 16. Security and robustness

JavaScript and bytecode are untrusted inputs. The verifier and compiler enforce
resource limits and never trust stack sizes, indices, jump targets, or metadata
without validation. Generated indirect calls target the registered helper table
or validated code entries. W^X is mandatory. Native code cannot embed raw
runtime objects whose lifetime is not represented by an artifact dependency.

The JIT preserves QuickJS memory limits and interrupts. Separate limits cover
snapshot bytes, pending jobs, compilation time, compiler memory, generated code,
metadata, and repeated tiering attempts. Backend faults disable the affected
function or runtime rather than corrupting interpreter state where recovery is
possible.

## 17. Upgrade workflow

For each QuickJS/rquickjs update:

1. Update the submodule and bindings normally.
2. Compile against the versioned JIT ABI and review any fingerprint mismatch.
3. Update only the ABI adapter for intentional bytecode or runtime changes.
4. Regenerate opcode metadata from the authoritative QuickJS opcode table.
5. Run verifier, opcode, differential, QuickJS, and Test262 suites.
6. Run the performance and memory comparison against the previous supported
   revision.
7. Publish the compatibility revision and fallback/coverage changes.

Compiler code may depend on generated stable opcode descriptions, never on
duplicated hand-maintained private C layouts. This workflow is the primary test
of the independent-crate boundary.

## 18. Delivery sequence

Implementation is staged to keep a correct interpreter fallback at every
commit:

1. Versioned ABI, feature gates, metrics, lifecycle, and no-op attachment.
2. Snapshot ownership, bytecode decoder, verifier, and generated opcode data.
3. Platform code allocator and executable-memory tests.
4. Tier 1 control flow, locals, constants, returns, and numeric operations.
5. Shared helpers, calls, objects, exceptions, interrupts, and ownership.
6. Background compilation, installation, cache, invalidation, and hot reload.
7. Loop feedback and Tier 1 OSR.
8. Full Tier 1 eligible-opcode coverage with differential and Test262 gates.
9. Tier 2 feedback, SSA, unboxing, guards, frame states, and deoptimization.
10. Tier 2 loop and call optimizations, side paths, and profitability policy.
11. `gpui-shell` integration, real workload tuning, cross-platform hardening,
    and the complete performance report.

Each stage has its own tests and retains automatic fallback. A later tier is not
allowed to weaken an earlier tier's correctness evidence.

## 19. Authoritative references

- QuickJS source in `sys/quickjs/quickjs.c`, `quickjs.h`, and
  `quickjs-opcode.h` at the repository-pinned revision.
- Navi JIT reference in `../navi-script/crates/vm/src/jit`.
- V8 Sparkplug: <https://v8.dev/blog/sparkplug>
- V8 Maglev: <https://v8.dev/blog/maglev>
- V8 background compilation: <https://v8.dev/blog/background-compilation>
- V8 lazy unlinking: <https://v8.dev/blog/lazy-unlinking>
- LuaJIT controls and optimization parameters:
  <https://luajit.org/ext_jit.html> and <https://luajit.org/running.html>
- OpenJDK OSR and tiering reference:
  <https://cr.openjdk.org/~thartmann/papers/2017-ManLang-Profile_Caching.pdf>
- Julia world age: <https://docs.julialang.org/en/v1.12-dev/manual/worldage/>
- Luau native code generation:
  <https://github.com/Roblox/creator-docs/blob/main/content/en-us/luau/native-code-gen.md>
- PyPy JIT papers: <https://doc.pypy.org/extradoc.html>
- Cranelift project status:
  <https://github.com/bytecodealliance/wasmtime/blob/main/cranelift/README.md>
- Apple JIT requirements:
  <https://developer.apple.com/documentation/apple-silicon/porting-just-in-time-compilers-to-apple-silicon>
- Microsoft Control Flow Guard:
  <https://learn.microsoft.com/en-us/windows/win32/secbp/control-flow-guard>

