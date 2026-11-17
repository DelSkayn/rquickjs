use rquickjs_sys as qjs;
use std::ffi::{c_void, CStr};

#[derive(Default)]
struct HookCounts {
    before: usize,
    after: usize,
    non_promise_argument: usize,
}

unsafe extern "C" fn promise_hook(
    _ctx: *mut qjs::JSContext,
    type_: qjs::JSPromiseHookType,
    promise: qjs::JSValue,
    _parent_promise: qjs::JSValue,
    opaque: *mut c_void,
) {
    let counts = unsafe { &mut *opaque.cast::<HookCounts>() };
    match type_ {
        qjs::JSPromiseHookType_JS_PROMISE_HOOK_BEFORE => {
            counts.before += 1;
            if !unsafe { qjs::JS_IsPromise(promise) } {
                counts.non_promise_argument += 1;
            }
        }
        qjs::JSPromiseHookType_JS_PROMISE_HOOK_AFTER => {
            counts.after += 1;
            if !unsafe { qjs::JS_IsPromise(promise) } {
                counts.non_promise_argument += 1;
            }
        }
        _ => {}
    }
}

unsafe fn evaluate(ctx: *mut qjs::JSContext, source: &CStr) {
    let filename = c"<promise-hook-test>";
    let source_len = source
        .to_bytes()
        .len()
        .try_into()
        .expect("JavaScript source length fits size_t");
    let result = unsafe {
        qjs::JS_Eval(
            ctx,
            source.as_ptr(),
            source_len,
            filename.as_ptr(),
            qjs::JS_EVAL_TYPE_GLOBAL as i32,
        )
    };
    let is_exception = unsafe { qjs::JS_IsException(result) };
    unsafe { qjs::JS_FreeValue(ctx, result) };
    assert!(!is_exception, "JavaScript evaluation failed");
}

unsafe fn execute_pending_jobs(rt: *mut qjs::JSRuntime) {
    while unsafe { qjs::JS_IsJobPending(rt) } {
        let mut ctx = std::ptr::null_mut();
        let result = unsafe { qjs::JS_ExecutePendingJob(rt, &mut ctx) };
        assert_eq!(result, 1, "pending Promise job failed");
    }
}

#[test]
fn hooks_run_around_then_and_await_continuations() {
    unsafe {
        let rt = qjs::JS_NewRuntime();
        assert!(!rt.is_null());

        let mut counts = Box::<HookCounts>::default();
        qjs::JS_SetPromiseHook(
            rt,
            Some(promise_hook),
            (&mut *counts as *mut HookCounts).cast(),
        );

        let ctx = qjs::JS_NewContext(rt);
        assert!(!ctx.is_null());

        for source in [
            c"Promise.resolve().then(() => {})",
            c"(async () => { await Promise.resolve(); })()",
        ] {
            evaluate(ctx, source);
            execute_pending_jobs(rt);

            assert_eq!(counts.before, 1);
            assert_eq!(counts.after, 1);
            assert_eq!(counts.non_promise_argument, 0);
            *counts = HookCounts::default();
        }

        qjs::JS_FreeContext(ctx);
        qjs::JS_FreeRuntime(rt);
    }
}
