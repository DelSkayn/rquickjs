use rquickjs_sys::*;
use std::{
    ptr,
    sync::atomic::{AtomicUsize, Ordering},
};

static CALLS: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn job(_: *mut JSContext, _: i32, _: *mut JSValue) -> JSValue {
    CALLS.fetch_add(1, Ordering::SeqCst);
    JS_UNDEFINED
}

#[test]
fn discard_jobs_preserves_runtime() {
    unsafe {
        let rt = JS_NewRuntime();
        assert!(!rt.is_null());
        let ctx = JS_NewContext(rt);
        assert!(!ctx.is_null());
        assert_eq!(JS_DiscardPendingJobs(rt), 0);
        for _ in 0..2 {
            assert_eq!(JS_EnqueueJob(ctx, Some(job), 0, ptr::null_mut()), 0);
        }
        assert_eq!(JS_DiscardPendingJobs(rt), 2);
        assert!(!JS_IsJobPending(rt));
        assert_eq!(CALLS.load(Ordering::SeqCst), 0);
        assert_eq!(JS_DiscardPendingJobs(rt), 0);
        assert_eq!(JS_EnqueueJob(ctx, Some(job), 0, ptr::null_mut()), 0);
        let mut job_ctx = ptr::null_mut();
        assert_eq!(JS_ExecutePendingJob(rt, &mut job_ctx), 1);
        assert_eq!(job_ctx, ctx);
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(JS_ExecutePendingJob(rt, &mut job_ctx), 0);
        JS_FreeContext(ctx);
        JS_FreeRuntime(rt);
    }
}

struct FinalizerState {
    ctx: *mut JSContext,
    calls: usize,
    finalizers: usize,
    enqueue_result: i32,
}

unsafe extern "C" fn finalizer_job(ctx: *mut JSContext, _: i32, _: *mut JSValue) -> JSValue {
    let state = &mut *JS_GetRuntimeOpaque(JS_GetRuntime(ctx)).cast::<FinalizerState>();
    state.calls += 1;
    JS_UNDEFINED
}

unsafe extern "C" fn enqueue_on_finalize(rt: *mut JSRuntime, _: JSValue) {
    let state = &mut *JS_GetRuntimeOpaque(rt).cast::<FinalizerState>();
    state.finalizers += 1;
    state.enqueue_result = JS_EnqueueJob(state.ctx, Some(finalizer_job), 0, ptr::null_mut());
}

#[test]
fn discard_jobs_leaves_finalizer_jobs_pending() {
    unsafe {
        let rt = JS_NewRuntime();
        assert!(!rt.is_null());
        let ctx = JS_NewContext(rt);
        assert!(!ctx.is_null());
        let mut state = FinalizerState {
            ctx,
            calls: 0,
            finalizers: 0,
            enqueue_result: -1,
        };
        JS_SetRuntimeOpaque(rt, ptr::addr_of_mut!(state).cast());
        let mut class_id = 0;
        JS_NewClassID(rt, &mut class_id);
        let class = JSClassDef {
            class_name: c"EnqueueOnFinalize".as_ptr(),
            finalizer: Some(enqueue_on_finalize),
            gc_mark: None,
            call: None,
            exotic: ptr::null_mut(),
        };
        assert_eq!(JS_NewClass(rt, class_id, &class), 0);

        for count in 1..=2 {
            let calls = state.calls;
            let finalizers = state.finalizers;
            for _ in 0..count {
                let mut arg = JS_NewObjectClass(ctx, class_id);
                assert!(!JS_IsException(arg));
                assert_eq!(JS_EnqueueJob(ctx, Some(finalizer_job), 1, &mut arg), 0);
                JS_FreeValue(ctx, arg);
            }
            assert_eq!(JS_DiscardPendingJobs(rt), count as size_t);
            assert_eq!(state.finalizers, finalizers + count);
            assert_eq!(state.enqueue_result, 0);
            assert_eq!(state.calls, calls);
            assert!(JS_IsJobPending(rt));

            let mut job_ctx = ptr::null_mut();
            for _ in 0..count {
                assert_eq!(JS_ExecutePendingJob(rt, &mut job_ctx), 1);
                assert_eq!(job_ctx, ctx);
            }
            assert_eq!(state.calls, calls + count);
            assert_eq!(JS_ExecutePendingJob(rt, &mut job_ctx), 0);
            assert_eq!(JS_DiscardPendingJobs(rt), 0);
        }

        JS_SetRuntimeOpaque(rt, ptr::null_mut());
        JS_FreeContext(ctx);
        JS_FreeRuntime(rt);
    }
}
