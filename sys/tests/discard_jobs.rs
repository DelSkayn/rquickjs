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
