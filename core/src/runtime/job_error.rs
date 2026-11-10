//! Exceptions thrown by queued jobs.
//!
//! QuickJS keeps a failing job's exception in a runtime-wide slot that the next
//! job can overwrite or consume. Every drain therefore catches the exception
//! immediately and moves it into a table owned by the runtime. [`JobError`] is a
//! `Send` token for one of those entries; [`JobError::into_value`] takes the
//! value back out with a context from the same runtime.

use alloc::vec::Vec;
use core::{
    fmt,
    mem::{self, MaybeUninit},
    result::Result as StdResult,
    sync::atomic::{AtomicU64, Ordering},
};

use super::opaque::Opaque;
use crate::{qjs, Ctx, Error, Persistent, Result, Value};

/// Process-wide ids so a token can never resolve to another runtime's entry.
static NEXT_JOB_ERROR_ID: AtomicU64 = AtomicU64::new(0);

/// A failed job's exception, kept inside its runtime.
///
/// Returned by [`Ctx::drain_jobs`](crate::Ctx::drain_jobs) and, with the
/// `"futures"` feature, by `AsyncContext::async_with`, `AsyncRuntime::idle`
/// and `AsyncRuntime::drive`.
///
/// The token carries no JavaScript value and is `Send`: the exception stays in
/// a table owned by the runtime until [`JobError::into_value`] takes it out.
/// Its id is process-wide unique, so it cannot name another runtime's
/// exception. Consume it before driving the runtime again; the next drain
/// releases unread exceptions.
pub struct JobError {
    id: u64,
}

impl fmt::Debug for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JobError")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "a queued job threw an exception".fmt(f)
    }
}

impl core::error::Error for JobError {}

impl JobError {
    /// Take the exception out of the runtime.
    ///
    /// `ctx` must belong to the runtime the job ran in — not necessarily the
    /// same context. Ids are process-wide unique, so a context from another
    /// runtime cannot resolve to a different exception; [`Error::JobErrorUnavailable`]
    /// is returned instead. It is also returned if the exception was already
    /// released by a later drain, or the runtime was dropped.
    pub fn into_value<'js>(self, ctx: &Ctx<'js>) -> Result<Value<'js>> {
        // SAFETY: a `Ctx` is only usable while the runtime lock is held, which
        // is what guards the runtime-owned table.
        let entry = unsafe { ctx.get_opaque().take_job_error(self.id) }?;
        entry.value.restore(ctx)
    }
}

pub(crate) struct Entry {
    id: u64,
    // SAFETY: `Persistent` erases the runtime borrow to `'static`. Entries are
    // dropped in `Opaque::clear` before `JS_FreeRuntime`, and are only handed
    // out by `JobErrors::take` with a live `Ctx` from the same runtime, which
    // `Persistent::restore` verifies.
    value: Persistent<Value<'static>>,
}

/// Runtime-owned table of failed job exceptions.
///
/// Every access happens while the runtime lock is held, so this needs no lock
/// of its own; `Opaque` keeps it in an `UnsafeCell` like its other
/// runtime-thread-only state.
#[derive(Default)]
pub(crate) struct JobErrors {
    entries: Vec<Entry>,
}

impl JobErrors {
    pub(crate) fn capture<'js>(&mut self, ctx: &Ctx<'js>, value: Value<'js>) -> JobError {
        let id = NEXT_JOB_ERROR_ID.fetch_add(1, Ordering::Relaxed);
        // `Persistent::save` erases the borrow and records the owning runtime.
        self.entries.push(Entry {
            id,
            value: Persistent::save(ctx, value),
        });
        JobError { id }
    }

    pub(crate) fn take(&mut self, id: u64) -> Result<Entry> {
        let idx = self
            .entries
            .iter()
            .position(|e| e.id == id)
            .ok_or(Error::JobErrorUnavailable)?;
        Ok(self.entries.swap_remove(idx))
    }

    /// Move every unconsumed entry out so the caller can drop them after the
    /// borrow of `self` ends (a finalizer may re-enter `Opaque`).
    pub(crate) fn take_all(&mut self) -> Vec<Entry> {
        mem::take(&mut self.entries)
    }
}

/// Execute one pending job, retaining the exception if it fails.
///
/// # Safety
/// `rt` must point to a live runtime whose lock is held.
pub(crate) unsafe fn execute_pending_job(rt: *mut qjs::JSRuntime) -> StdResult<bool, JobError> {
    let mut ctx_ptr = MaybeUninit::<*mut qjs::JSContext>::uninit();
    match qjs::JS_ExecutePendingJob(rt, ctx_ptr.as_mut_ptr()) {
        0 => Ok(false),
        1 => Ok(true),
        _ => {
            // QuickJS always reports the failing context on its error path.
            // Guard anyway so library code never builds a `Ctx` from null.
            let ctx_ptr = ctx_ptr.assume_init();
            if ctx_ptr.is_null() {
                return Ok(true);
            }
            Err(failed_job(rt, ctx_ptr))
        }
    }
}

/// # Safety
/// `rt` must point to a live runtime whose lock is held; `ctx` is the context
/// `JS_ExecutePendingJob` reported for a failed job.
unsafe fn failed_job(rt: *mut qjs::JSRuntime, ctx: *mut qjs::JSContext) -> JobError {
    let opaque = Opaque::from_runtime_ptr(rt);
    let panic = opaque.take_panic();

    // `JS_ExecutePendingJob` hands back a borrowed context; dup it for the guard.
    let ctx = Ctx::from_ptr(ctx);
    let exception = qjs::JS_GetException(ctx.as_ptr());

    // A panicking Rust callback throws a `JS_TAG_EXCEPTION` sentinel. JS cannot
    // fabricate that tag, so it is the only failure the stashed payload belongs
    // to; resume it here instead of turning it into a JavaScript value.
    let is_panic = qjs::JS_VALUE_GET_NORM_TAG(exception) == qjs::JS_TAG_EXCEPTION;
    match panic {
        Some(payload) if is_panic => crate::util::resume_unwind(payload),
        Some(_) => {} // JS caught the synthetic exception; don't resume it later.
        None => {}
    }

    let value = Value::from_js_value(ctx.clone(), exception);
    unsafe { opaque.capture_job_error(&ctx, value) }
}

#[cfg(test)]
mod test {
    use super::JobError;
    use crate::{function::Func, Context, Ctx, Error, Promise, Runtime};

    fn message_of(ctx: &Ctx<'_>, err: JobError) -> String {
        err.into_value(ctx)
            .unwrap()
            .as_object()
            .unwrap()
            .get::<_, String>("message")
            .unwrap()
    }

    #[test]
    fn drain_returns_the_failed_microtask_exception() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            ctx.eval::<(), _>("queueMicrotask(() => { throw new Error('foobar'); })")
                .unwrap();
            let err = ctx.drain_jobs().unwrap_err();
            assert_eq!(message_of(&ctx, err), "foobar");
        });
    }

    #[test]
    fn later_jobs_cannot_clobber_a_retained_exception() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            ctx.eval::<(), _>(
                "queueMicrotask(() => { throw new Error('first'); });\
                 queueMicrotask(() => { throw new Error('second'); });",
            )
            .unwrap();

            let first = ctx.drain_jobs().unwrap_err();
            ctx.globals()
                .set("a", first.into_value(&ctx).unwrap())
                .unwrap();
            let second = ctx.drain_jobs().unwrap_err();
            ctx.globals()
                .set("b", second.into_value(&ctx).unwrap())
                .unwrap();
            assert_eq!(ctx.eval::<String, _>("a.message").unwrap(), "first");
            assert_eq!(ctx.eval::<String, _>("b.message").unwrap(), "second");
        });
    }

    #[test]
    fn an_unread_handle_is_released_by_the_next_drain() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            ctx.eval::<(), _>("queueMicrotask(() => { throw new Error('first'); })")
                .unwrap();
            let err = ctx.drain_jobs().unwrap_err();

            ctx.eval::<(), _>("queueMicrotask(() => { globalThis.ok = 1; })")
                .unwrap();
            ctx.drain_jobs().unwrap();

            assert!(matches!(
                err.into_value(&ctx),
                Err(Error::JobErrorUnavailable)
            ));
            assert_eq!(ctx.eval::<i32, _>("globalThis.ok").unwrap(), 1);
        });
    }

    #[test]
    fn a_stale_pending_exception_is_not_a_job_error() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let _ = ctx.eval::<(), _>("throw new Error('stale')");
            ctx.eval::<(), _>("queueMicrotask(() => { globalThis.ran = 1; })")
                .unwrap();
            ctx.drain_jobs().unwrap();
            assert_eq!(ctx.eval::<i32, _>("globalThis.ran").unwrap(), 1);
        });
    }

    #[test]
    fn into_value_works_from_another_context_of_the_same_runtime() {
        let rt = Runtime::new().unwrap();
        let err = {
            let ctx1 = Context::full(&rt).unwrap();
            ctx1.with(|ctx| {
                ctx.eval::<(), _>("queueMicrotask(() => { throw new Error('shared'); })")
                    .unwrap();
                ctx.drain_jobs().unwrap_err()
            })
            // The failing context is dropped here; the retained value keeps it alive.
        };

        let ctx2 = Context::full(&rt).unwrap();
        ctx2.with(|ctx| {
            ctx.globals()
                .set("err", err.into_value(&ctx).unwrap())
                .unwrap();
            assert_eq!(
                ctx.eval::<std::string::String, _>("err.message").unwrap(),
                "shared"
            );
        });
    }

    #[test]
    fn a_stale_handle_cannot_resolve_in_a_new_runtime() {
        let err = {
            let rt1 = Runtime::new().unwrap();
            let ctx1 = Context::full(&rt1).unwrap();
            ctx1.with(|ctx| {
                ctx.eval::<(), _>("queueMicrotask(() => { throw new Error('old'); })")
                    .unwrap();
                ctx.drain_jobs().unwrap_err()
            })
        };

        let rt2 = Runtime::new().unwrap();
        let ctx2 = Context::full(&rt2).unwrap();
        ctx2.with(|ctx| {
            // The new runtime has its own failure, so a per-runtime id would
            // have collided here and handed back the wrong exception.
            ctx.eval::<(), _>("queueMicrotask(() => { throw new Error('new'); })")
                .unwrap();
            let _ = ctx.drain_jobs().unwrap_err();
            assert!(matches!(
                err.into_value(&ctx),
                Err(Error::JobErrorUnavailable)
            ));
        });
    }

    #[test]
    fn into_value_rejects_an_unrelated_runtime() {
        let rt1 = Runtime::new().unwrap();
        let ctx1 = Context::full(&rt1).unwrap();
        let err = ctx1.with(|ctx| {
            ctx.eval::<(), _>("queueMicrotask(() => { throw 3; })")
                .unwrap();
            ctx.drain_jobs().unwrap_err()
        });

        let rt2 = Runtime::new().unwrap();
        let ctx2 = Context::full(&rt2).unwrap();
        ctx2.with(|ctx| {
            // A process-wide id cannot resolve in another runtime's table, so
            // no exception is returned rather than the wrong one.
            assert!(matches!(
                err.into_value(&ctx),
                Err(Error::JobErrorUnavailable)
            ));
        });
    }

    #[test]
    fn repeated_unread_errors_are_freed() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            for _ in 0..4 {
                ctx.eval::<(), _>("queueMicrotask(() => { throw new Error('x'); })")
                    .unwrap();
                let _ = ctx.drain_jobs().unwrap_err();
            }
            // Each drain frees the previous unread exception. A double free or
            // a value leaking past `JS_FreeRuntime` would abort the process.
            ctx.drain_jobs().unwrap();
        });
    }

    #[test]
    fn an_unconsumed_handle_survives_runtime_teardown() {
        let err = {
            let rt = Runtime::new().unwrap();
            let ctx = Context::full(&rt).unwrap();
            ctx.with(|ctx| {
                ctx.eval::<(), _>("queueMicrotask(() => { throw new Error('leak'); })")
                    .unwrap();
                ctx.drain_jobs().unwrap_err()
            })
        };
        let _ = err;
    }

    #[test]
    fn promise_finish_rethrows_a_failed_job() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let promise: Promise = ctx.eval("new Promise(() => {})").unwrap();
            ctx.eval::<(), _>("queueMicrotask(() => { throw new Error('from job'); })")
                .unwrap();
            assert!(matches!(promise.finish::<()>(), Err(Error::Exception)));
            ctx.globals().set("err", ctx.catch()).unwrap();
            assert_eq!(ctx.eval::<String, _>("err.message").unwrap(), "from job");
        });
    }

    // wasm uses `panic=abort`, so `catch_unwind` cannot stash a panic payload
    // and `should_panic` tests are ignored by libtest there.
    #[cfg_attr(target_arch = "wasm32", ignore)]
    #[test]
    #[should_panic(expected = "boom")]
    fn a_panicking_job_is_resumed() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            ctx.globals()
                .set("boom", Func::from(|| -> () { panic!("boom") }))
                .unwrap();
            ctx.eval::<(), _>("queueMicrotask(boom)").unwrap();
            let _ = ctx.drain_jobs();
        });
    }

    #[cfg_attr(target_arch = "wasm32", ignore)]
    #[test]
    fn a_caught_panic_is_not_resumed_at_a_later_failure() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            ctx.globals()
                .set("boom", Func::from(|| -> () { panic!("boom") }))
                .unwrap();
            ctx.eval::<(), _>("queueMicrotask(() => { try { boom() } catch (e) {} })")
                .unwrap();
            ctx.drain_jobs().unwrap();

            ctx.eval::<(), _>("queueMicrotask(() => { throw new Error('later'); })")
                .unwrap();
            let err = ctx.drain_jobs().unwrap_err();
            assert_eq!(message_of(&ctx, err), "later");
        });
    }
}
