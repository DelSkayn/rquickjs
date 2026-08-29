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
