use rquickjs::{Context, Runtime};
use rquickjs_jit::{JitConfig, JitError, JitRuntime};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

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

#[test]
fn configuration_accepts_structured_diagnostic_and_metrics_callbacks() {
    let diagnostics = Arc::new(AtomicUsize::new(0));
    let observations = Arc::new(AtomicUsize::new(0));
    let diagnostic_count = Arc::clone(&diagnostics);
    let observation_count = Arc::clone(&observations);
    let config = JitConfig::builder()
        .diagnostic_callback(move |diagnostic| {
            let _ = diagnostic.kind();
            diagnostic_count.fetch_add(1, Ordering::Relaxed);
        })
        .metrics_observer(move |metrics| {
            let _ = metrics.native_enabled();
            observation_count.fetch_add(1, Ordering::Relaxed);
        })
        .build()
        .unwrap();

    let runtime = JitRuntime::builder().config(config).build().unwrap();
    assert_eq!(diagnostics.load(Ordering::Relaxed), 0);
    assert_eq!(observations.load(Ordering::Relaxed), 1);
    drop(runtime);
}

#[cfg(any(
    all(
        target_os = "macos",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(
        target_os = "windows",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
))]
#[test]
fn native_execution_is_available_on_supported_targets() {
    let runtime = JitRuntime::builder().build().expect("JIT runtime");
    assert!(runtime.jit().require_native().is_ok());
}

#[cfg(not(any(
    all(
        target_os = "macos",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(
        target_os = "windows",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
)))]
#[test]
fn native_execution_is_rejected_on_unsupported_targets() {
    let runtime = JitRuntime::builder().build().expect("JIT runtime");
    assert!(matches!(
        runtime.jit().require_native(),
        Err(JitError::UnsupportedPlatform)
    ));
}
