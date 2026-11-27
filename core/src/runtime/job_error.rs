//! Exceptions thrown by queued jobs.
//!
//! QuickJS keeps a failing job's exception in a runtime-wide slot that the next
//! job can overwrite or consume. Every drain therefore catches the exception
//! immediately and keeps a copy in the runtime. [`JobError`] is a `Send` token
//! for that copy; [`JobError::into_value`] clones it out with a context from
//! the same runtime.

use alloc::collections::VecDeque;
use core::{
    fmt,
    mem::MaybeUninit,
    result::Result as StdResult,
    sync::atomic::{AtomicU64, Ordering},
};

use super::opaque::RuntimeOpaque;
use crate::{qjs, Ctx, Error, Persistent, Result, Value};

/// Process-wide ids so a token can never resolve to another runtime's error.
static NEXT_JOB_ERROR_ID: AtomicU64 = AtomicU64::new(0);

/// Retain enough failures that concurrently driven contexts can each keep one.
/// The oldest is evicted once the queue is full, so memory stays bounded.
pub(crate) const MAX_JOB_ERRORS: usize = 32;

/// Bounded FIFO of failed-job exceptions.
///
/// Every access happens while the runtime lock is held, so this needs no lock
/// of its own.
pub(crate) struct JobErrors {
    entries: VecDeque<Entry>,
}

impl Default for JobErrors {
    fn default() -> Self {
        Self::with_capacity(MAX_JOB_ERRORS)
    }
}

impl JobErrors {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
        }
    }

    /// Push a failure, returning an evicted entry for the caller to drop after
    /// the borrow of `self` ends (a finalizer may re-enter `Opaque`).
    pub(crate) fn push(&mut self, entry: Entry) -> Option<Entry> {
        let evicted = if self.entries.len() == MAX_JOB_ERRORS {
            self.entries.pop_front()
        } else {
            None
        };
        self.entries.push_back(entry);
        evicted
    }

    /// Clone the exception with the given id out of the queue.
    pub(crate) fn get<'js>(&self, id: u64, ctx: &Ctx<'js>) -> Result<Value<'js>> {
        let entry = self
            .entries
            .iter()
            .find(|e| e.id == id)
            .ok_or(Error::JobErrorUnavailable)?;
        entry.value.clone().restore(ctx)
    }

    /// Move every entry out before `JS_FreeRuntime`.
    pub(crate) fn take_all(&mut self) -> VecDeque<Entry> {
        core::mem::take(&mut self.entries)
    }
}

/// A failed job's exception, kept inside its runtime.
///
/// Returned by [`Ctx::drain_jobs`](crate::Ctx::drain_jobs) and, with the
/// `"futures"` feature, by `AsyncContext::async_with`, `AsyncRuntime::idle`
/// and `AsyncRuntime::drive`.
///
/// The token carries no JavaScript value and is `Send`: the exception stays in
/// its runtime until it is evicted by an older-first FIFO (holding up to
/// [`MAX_JOB_ERRORS`](MAX_JOB_ERRORS) failures) or the runtime is dropped.
/// The value is cloned on extraction, so a token can be used more than once.
#[derive(Clone)]
pub struct JobError {
    pub(crate) id: u64,
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
    /// Clone the exception out of the runtime.
    ///
    /// `ctx` must belong to the runtime the job ran in — not necessarily the
    /// same context. Ids are process-wide unique, so a context from another
    /// runtime cannot resolve to a different exception; [`Error::JobErrorUnavailable`]
    /// is returned instead. It is also returned if the failure was evicted
    /// from the bounded FIFO, or the runtime was dropped.
    pub fn into_value<'js>(&self, ctx: &Ctx<'js>) -> Result<Value<'js>> {
        // SAFETY: a `Ctx` is only usable while the runtime lock is held, which
        // is what guards the runtime-owned queue.
        unsafe { ctx.get_runtime_opaque().take_job_error(self.id, ctx) }
    }
}

/// One retained exception.
pub(crate) struct Entry {
    pub(crate) id: u64,
    // SAFETY: `Persistent` erases the runtime borrow to `'static`. The entry is
    // dropped in `RuntimeOpaque::clear` or on eviction, before `JS_FreeRuntime`,
    // and `restore` rebinds a clone to a live `Ctx` from the same runtime.
    pub(crate) value: Persistent<Value<'static>>,
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
    let opaque = RuntimeOpaque::from_runtime_ptr(rt);
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
    let id = NEXT_JOB_ERROR_ID.fetch_add(1, Ordering::Relaxed);
    unsafe { opaque.capture_job_error(id, &ctx, value) }
}

#[cfg(test)]
mod test {
    use super::JobError;
    use crate::{function::Func, Context, Ctx, Error, Promise, Runtime};

    fn message_of(ctx: &Ctx<'_>, err: &JobError) -> String {
        err.clone()
            .into_value(ctx)
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
            assert_eq!(message_of(&ctx, &err), "foobar");
        });
    }

    #[test]
    fn a_token_can_be_used_more_than_once() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            ctx.eval::<(), _>("queueMicrotask(() => { throw new Error('twice'); })")
                .unwrap();
            let err = ctx.drain_jobs().unwrap_err();
            assert_eq!(message_of(&ctx, &err), "twice");
            assert_eq!(message_of(&ctx, &err), "twice");
        });
    }

    #[test]
    fn multiple_failures_are_all_retained() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            ctx.eval::<(), _>(
                "queueMicrotask(() => { throw new Error('first'); });\
                 queueMicrotask(() => { throw new Error('second'); });",
            )
            .unwrap();

            let first = ctx.drain_jobs().unwrap_err();
            let second = ctx.drain_jobs().unwrap_err();
            assert_eq!(message_of(&ctx, &first), "first");
            assert_eq!(message_of(&ctx, &second), "second");
        });
    }

    #[test]
    fn a_live_handle_survives_later_drains() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            ctx.eval::<(), _>("queueMicrotask(() => { throw new Error('first'); })")
                .unwrap();
            let err = ctx.drain_jobs().unwrap_err();

            ctx.eval::<(), _>("queueMicrotask(() => { globalThis.ok = 1; })")
                .unwrap();
            ctx.drain_jobs().unwrap();

            assert_eq!(message_of(&ctx, &err), "first");
            assert_eq!(ctx.eval::<i32, _>("globalThis.ok").unwrap(), 1);
        });
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn error_survives_a_drain_on_another_thread() {
        use std::thread;

        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            ctx.eval::<(), _>("queueMicrotask(() => { throw new Error('shared'); })")
                .unwrap();
        });
        let err = ctx.with(|ctx| ctx.drain_jobs()).unwrap_err();

        let ctx2 = ctx.clone();
        thread::scope(|scope| {
            scope.spawn(move || ctx2.with(|ctx| ctx.drain_jobs().unwrap()));
        });

        ctx.with(|ctx| assert_eq!(message_of(&ctx, &err), "shared"));
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
            assert!(matches!(
                err.into_value(&ctx),
                Err(Error::JobErrorUnavailable)
            ));
        });
    }

    #[test]
    fn oldest_errors_are_evicted_from_the_fifo() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let mut errors = Vec::new();
            for i in 0..super::MAX_JOB_ERRORS {
                ctx.eval::<(), _>(format!(
                    "queueMicrotask(() => {{ throw new Error('e{i}'); }})"
                ))
                .unwrap();
                errors.push(ctx.drain_jobs().unwrap_err());
            }
            // All retained so far.
            assert_eq!(message_of(&ctx, &errors[0]), "e0");

            ctx.eval::<(), _>("queueMicrotask(() => { throw new Error('new'); })")
                .unwrap();
            let newest = ctx.drain_jobs().unwrap_err();

            // The oldest was evicted, the rest survived.
            assert!(matches!(
                errors[0].into_value(&ctx),
                Err(Error::JobErrorUnavailable)
            ));
            assert_eq!(message_of(&ctx, &errors[1]), "e1");
            assert_eq!(message_of(&ctx, &newest), "new");
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
            assert_eq!(message_of(&ctx, &err), "later");
        });
    }
}
