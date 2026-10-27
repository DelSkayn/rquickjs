//! Promise futures observing interrupted jobs across the async job drains.
//!
//! Each test is bounded by a timeout, so a future that misses an interrupted
//! job fails instead of hanging. Interrupt handlers disarm themselves after
//! the first interrupt and are armed only once the looping job is queued.

use std::{
    panic::AssertUnwindSafe,
    sync::{
        atomic::{AtomicBool, Ordering::SeqCst},
        Arc, Mutex,
    },
    time::Duration,
};

use futures_rs::{poll, FutureExt};
use tokio::sync::{oneshot, Notify};

use crate::{promise::PromiseFuture, AsyncContext, AsyncRuntime, Ctx, Error, Module, Promise};

const TIMEOUT: Duration = Duration::from_secs(5);

/// A job that never settles its promise unless it is interrupted.
const LOOPING_JOB: &str = "Promise.resolve().then(() => { for (;;) {} })";

/// What runs between the interrupted job and the promise future's next poll.
#[derive(Clone, Copy)]
struct Case {
    /// Jobs queued behind the looping job, run by the same drain.
    later: &'static str,
    /// Whether host code throws and catches before awaiting the promise.
    host_catch: bool,
    /// How many of the later jobs fail.
    later_failures: usize,
}

const LATER_THROW: Case = Case {
    later: "queueMicrotask(() => { throw 2; });",
    host_catch: false,
    later_failures: 1,
};

const LATER_CATCH: Case = Case {
    later: "queueMicrotask(() => { try { throw 2; } catch {} });",
    host_catch: false,
    later_failures: 0,
};

const LATER_REJECTION: Case = Case {
    later: "Promise.reject(1).then(() => {}).catch(() => {});",
    host_catch: false,
    later_failures: 0,
};

const LATER_TRY_AWAIT: Case = Case {
    later: "(async () => { try { await Promise.reject(1); } catch {} })();",
    host_catch: false,
    later_failures: 0,
};

const HOST_CATCH: Case = Case {
    later: "",
    host_catch: true,
    later_failures: 0,
};

/// Install an interrupt handler that interrupts once when armed.
async fn one_shot_interrupt(rt: &AsyncRuntime) -> Arc<AtomicBool> {
    let armed = Arc::new(AtomicBool::new(false));
    let handler_armed = armed.clone();
    rt.set_interrupt_handler(Some(Box::new(move || handler_armed.swap(false, SeqCst))))
        .await;
    armed
}

/// Report job failures as whether they were uncatchable, notifying on each.
async fn report_job_errors(rt: &AsyncRuntime) -> (Arc<Mutex<Vec<bool>>>, Arc<Notify>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let reported = Arc::new(Notify::new());
    {
        let seen = seen.clone();
        let reported = reported.clone();
        rt.set_job_error_handler(Some(Box::new(move |_, error| {
            seen.lock().unwrap().push(error.is_uncatchable_error());
            reported.notify_one();
        })))
        .await;
    }
    (seen, reported)
}

fn spawn_drive(rt: &AsyncRuntime) {
    #[cfg(feature = "parallel")]
    tokio::spawn(rt.drive());
    #[cfg(not(feature = "parallel"))]
    tokio::task::spawn_local(rt.drive());
}

/// Queue the looping job, then `later`, keeping the looping job's promise in
/// `globalThis.p`.
fn script(later: &str) -> String {
    format!("globalThis.p = {LOOPING_JOB}; {later} p")
}

fn host_throw_and_catch(ctx: &Ctx<'_>) {
    let _ = ctx.eval::<(), _>("throw 1").map_err(|_| ctx.catch());
}

/// Await a promise that an interrupted job never settles. It must fail with
/// the uncatchable error and leave nothing pending once caught.
async fn assert_observes_interrupt<'js>(ctx: &Ctx<'js>, future: PromiseFuture<'js, ()>) {
    let res = future.await;
    assert!(matches!(res, Err(Error::Exception)));
    assert!(ctx.catch().is_uncatchable_error());
    assert!(!ctx.has_exception());
    let sum: i32 = ctx.eval("1 + 1").unwrap();
    assert_eq!(sum, 2);
}

/// The interrupt is retained exactly once, ahead of any later failures.
fn assert_retained_once(ctx: &Ctx<'_>, later_failures: usize) {
    assert!(ctx.take_pending_job_error().unwrap().is_uncatchable_error());
    assert_eq!(ctx.take_pending_job_error_overflow(), later_failures as u64);
}

/// The observer is already pending in the `async_with` future when the
/// driver's drain interrupts the job.
async fn async_with_case(rt: &AsyncRuntime, ctx: &AsyncContext, case: Case) {
    let armed = one_shot_interrupt(rt).await;
    tokio::time::timeout(
        TIMEOUT,
        ctx.async_with(async |ctx| {
            let promise: Promise = ctx.eval(script(case.later)).unwrap();
            let mut future = promise.into_future::<()>();
            assert!(poll!(&mut future).is_pending());
            armed.store(true, SeqCst);
            if case.host_catch {
                // Let the driver drain the jobs before host code catches.
                tokio::task::yield_now().await;
                assert!(!armed.load(SeqCst));
                host_throw_and_catch(&ctx);
            }
            assert_observes_interrupt(&ctx, future).await;
            assert!(!armed.load(SeqCst));
            assert_retained_once(&ctx, case.later_failures);
        }),
    )
    .await
    .expect("the promise future hung on an interrupted job");
}

/// The drive task interrupts the job before the observer exists.
async fn drive_case(rt: &AsyncRuntime, ctx: &AsyncContext, case: Case) {
    let armed = one_shot_interrupt(rt).await;
    let (seen, reported) = report_job_errors(rt).await;
    spawn_drive(rt);

    ctx.async_with(async |ctx| {
        ctx.eval::<(), _>(script(case.later)).unwrap();
        armed.store(true, SeqCst);
        // Wake the drive task so its drain runs the queued jobs.
        ctx.spawn(async move {});
    })
    .await;

    // The handler runs mid-drain with the runtime lock held, so the drive
    // task finishes the later jobs before the observer below can run.
    tokio::time::timeout(TIMEOUT, reported.notified())
        .await
        .expect("the drive task did not run the looping job");
    assert!(!armed.load(SeqCst));

    tokio::time::timeout(
        TIMEOUT,
        ctx.async_with(async |ctx| {
            if case.host_catch {
                host_throw_and_catch(&ctx);
            }
            let promise: Promise = ctx.globals().get("p").unwrap();
            assert_observes_interrupt(&ctx, promise.into_future()).await;
        }),
    )
    .await
    .expect("the promise future hung on an interrupted job");

    let mut expected = vec![true];
    expected.resize(1 + case.later_failures, false);
    assert_eq!(*seen.lock().unwrap(), expected);
}

/// `idle` interrupts the job before the observer exists.
async fn idle_case(rt: &AsyncRuntime, ctx: &AsyncContext, case: Case) {
    let armed = one_shot_interrupt(rt).await;
    ctx.with(|ctx| {
        ctx.eval::<(), _>(script(case.later)).unwrap();
        armed.store(true, SeqCst);
    })
    .await;

    tokio::time::timeout(TIMEOUT, rt.idle())
        .await
        .expect("idle hung");
    assert!(!armed.load(SeqCst));

    tokio::time::timeout(
        TIMEOUT,
        ctx.async_with(async |ctx| {
            if case.host_catch {
                host_throw_and_catch(&ctx);
            }
            let promise: Promise = ctx.globals().get("p").unwrap();
            assert_observes_interrupt(&ctx, promise.into_future()).await;
            assert_retained_once(&ctx, case.later_failures);
        }),
    )
    .await
    .expect("the promise future hung on an interrupted job");
}

/// Outcome reported by a spawned observer.
#[derive(Debug, PartialEq)]
struct Outcome {
    exception: bool,
    uncatchable: bool,
    pending_after_catch: bool,
}

const OBSERVED: Outcome = Outcome {
    exception: true,
    uncatchable: true,
    pending_after_catch: false,
};

/// Spawn a future that queues the looping job and polls a promise future for
/// it once before arming the interrupt, so the promise future is already
/// pending inside a spawned future when the job is interrupted. No later job
/// settles anything or wakes the spawned future.
fn spawn_pending_observer(ctx: &Ctx<'_>, armed: Arc<AtomicBool>) -> oneshot::Receiver<Outcome> {
    let (tx, rx) = oneshot::channel();
    let observer_ctx = ctx.clone();
    ctx.spawn(async move {
        let ctx = observer_ctx;
        let promise: Promise = ctx.eval(LOOPING_JOB).unwrap();
        let mut future = promise.into_future::<()>();
        assert!(poll!(&mut future).is_pending());
        armed.store(true, SeqCst);
        let res = future.await;
        let exception = matches!(res, Err(Error::Exception));
        let uncatchable = ctx.catch().is_uncatchable_error();
        let _ = tx.send(Outcome {
            exception,
            uncatchable,
            pending_after_catch: ctx.has_exception(),
        });
    });
    rx
}

async_test_case!(async_with_observes_interrupt_despite_later_throw => (rt, ctx) {
    async_with_case(&rt, &ctx, LATER_THROW).await;
});

async_test_case!(async_with_observes_interrupt_despite_later_catch => (rt, ctx) {
    async_with_case(&rt, &ctx, LATER_CATCH).await;
});

async_test_case!(async_with_observes_interrupt_despite_later_rejection => (rt, ctx) {
    async_with_case(&rt, &ctx, LATER_REJECTION).await;
});

async_test_case!(async_with_observes_interrupt_despite_later_try_await => (rt, ctx) {
    async_with_case(&rt, &ctx, LATER_TRY_AWAIT).await;
});

async_test_case!(async_with_observes_interrupt_despite_host_catch => (rt, ctx) {
    async_with_case(&rt, &ctx, HOST_CATCH).await;
});

async_test_case!(drive_observes_interrupt_despite_later_throw => (rt, ctx) {
    drive_case(&rt, &ctx, LATER_THROW).await;
});

async_test_case!(drive_observes_interrupt_despite_later_catch => (rt, ctx) {
    drive_case(&rt, &ctx, LATER_CATCH).await;
});

async_test_case!(drive_observes_interrupt_despite_later_rejection => (rt, ctx) {
    drive_case(&rt, &ctx, LATER_REJECTION).await;
});

async_test_case!(drive_observes_interrupt_despite_later_try_await => (rt, ctx) {
    drive_case(&rt, &ctx, LATER_TRY_AWAIT).await;
});

async_test_case!(drive_observes_interrupt_despite_host_catch => (rt, ctx) {
    drive_case(&rt, &ctx, HOST_CATCH).await;
});

async_test_case!(idle_observes_interrupt_despite_later_throw => (rt, ctx) {
    idle_case(&rt, &ctx, LATER_THROW).await;
});

async_test_case!(idle_observes_interrupt_despite_later_catch => (rt, ctx) {
    idle_case(&rt, &ctx, LATER_CATCH).await;
});

async_test_case!(idle_observes_interrupt_despite_later_rejection => (rt, ctx) {
    idle_case(&rt, &ctx, LATER_REJECTION).await;
});

async_test_case!(idle_observes_interrupt_despite_later_try_await => (rt, ctx) {
    idle_case(&rt, &ctx, LATER_TRY_AWAIT).await;
});

async_test_case!(idle_observes_interrupt_despite_host_catch => (rt, ctx) {
    idle_case(&rt, &ctx, HOST_CATCH).await;
});

async_test_case!(async_with_wakes_spawned_observer => (rt, ctx) {
    let armed = one_shot_interrupt(&rt).await;
    tokio::time::timeout(
        TIMEOUT,
        ctx.async_with(async |ctx| {
            let outcome = spawn_pending_observer(&ctx, armed.clone()).await.unwrap();
            assert_eq!(outcome, OBSERVED);
            assert!(!armed.load(SeqCst));
            assert_retained_once(&ctx, 0);
        }),
    )
    .await
    .expect("the spawned promise future hung on an interrupted job");
});

async_test_case!(drive_wakes_spawned_observer => (rt, ctx) {
    let armed = one_shot_interrupt(&rt).await;
    spawn_drive(&rt);
    let observer = ctx.with(|ctx| spawn_pending_observer(&ctx, armed.clone())).await;
    let outcome = tokio::time::timeout(TIMEOUT, observer)
        .await
        .expect("the spawned promise future hung on an interrupted job")
        .unwrap();
    assert_eq!(outcome, OBSERVED);
    assert!(!armed.load(SeqCst));
    ctx.with(|ctx| {
        assert!(!ctx.has_exception());
        assert_retained_once(&ctx, 0);
    })
    .await;
});

async_test_case!(idle_wakes_spawned_observer => (rt, ctx) {
    let armed = one_shot_interrupt(&rt).await;
    let mut observer = ctx.with(|ctx| spawn_pending_observer(&ctx, armed.clone())).await;
    tokio::time::timeout(TIMEOUT, rt.idle())
        .await
        .expect("the spawned promise future hung on an interrupted job");
    assert_eq!(observer.try_recv().unwrap(), OBSERVED);
    assert!(!armed.load(SeqCst));
    ctx.with(|ctx| {
        assert!(!ctx.has_exception());
        assert_retained_once(&ctx, 0);
    })
    .await;
});

async_test_case!(async_drains_leave_no_pending_exception => (rt, ctx) {
    let armed = one_shot_interrupt(&rt).await;

    // `async_with` driver.
    tokio::time::timeout(
        TIMEOUT,
        ctx.async_with(async |ctx| {
            ctx.eval::<(), _>(LOOPING_JOB).unwrap();
            armed.store(true, SeqCst);
            tokio::task::yield_now().await;
            assert!(!armed.load(SeqCst));
            assert!(!ctx.has_exception());
        }),
    )
    .await
    .expect("async_with hung");

    // `idle`.
    ctx.with(|ctx| {
        ctx.eval::<(), _>(LOOPING_JOB).unwrap();
        armed.store(true, SeqCst);
    })
    .await;
    tokio::time::timeout(TIMEOUT, rt.idle())
        .await
        .expect("idle hung");
    assert!(!armed.load(SeqCst));

    ctx.with(|ctx| {
        assert!(!ctx.has_exception());
        let sum: i32 = ctx.eval("1 + 1").unwrap();
        assert_eq!(sum, 2);
        let (_, promise) = Module::declare(ctx.clone(), "m", "export const x = 1;")
            .unwrap()
            .eval()
            .unwrap();
        promise.finish::<()>().unwrap();
    })
    .await;
});

async_test_case!(observed_interrupt_does_not_poison_later_futures => (rt, ctx) {
    let armed = one_shot_interrupt(&rt).await;
    tokio::time::timeout(
        TIMEOUT,
        ctx.async_with(async |ctx| {
            let promise: Promise = ctx.eval(LOOPING_JOB).unwrap();
            armed.store(true, SeqCst);
            assert_observes_interrupt(&ctx, promise.into_future()).await;

            // Created and awaited after the interrupt was observed, and
            // pending when first polled.
            let promise: Promise = ctx.eval("Promise.resolve().then(() => 42)").unwrap();
            assert_eq!(promise.into_future::<i32>().await.unwrap(), 42);
            let promise: Promise = ctx
                .eval("Promise.resolve().then(() => { throw 7; })")
                .unwrap();
            assert!(matches!(promise.into_future::<()>().await, Err(Error::Exception)));
            assert_eq!(ctx.catch().as_int(), Some(7));
        }),
    )
    .await
    .expect("a promise future hung");

    // Nor in a later `async_with`.
    tokio::time::timeout(
        TIMEOUT,
        ctx.async_with(async |ctx| {
            let promise: Promise = ctx.eval("Promise.resolve().then(() => 42)").unwrap();
            assert_eq!(promise.into_future::<i32>().await.unwrap(), 42);
            assert!(!ctx.has_exception());
        }),
    )
    .await
    .expect("a promise future hung");
});

async_test_case!(unobserved_interrupt_fails_at_most_one_later_future => (rt, ctx) {
    let armed = one_shot_interrupt(&rt).await;
    ctx.with(|ctx| {
        ctx.eval::<(), _>(LOOPING_JOB).unwrap();
        armed.store(true, SeqCst);
    })
    .await;
    tokio::time::timeout(TIMEOUT, rt.idle())
        .await
        .expect("idle hung");
    assert!(!armed.load(SeqCst));

    tokio::time::timeout(
        TIMEOUT,
        ctx.async_with(async |ctx| {
            // Settled promises are unaffected and leave the interrupt unobserved.
            let promise: Promise = ctx.eval("Promise.resolve(5)").unwrap();
            assert_eq!(promise.into_future::<i32>().await.unwrap(), 5);

            // The first pending promise future observes the interrupt, even
            // though its own promise is unrelated (the documented coarse scope).
            let promise: Promise = ctx.eval("Promise.resolve().then(() => 1)").unwrap();
            assert!(matches!(promise.into_future::<i32>().await, Err(Error::Exception)));
            assert!(ctx.catch().is_uncatchable_error());

            // Later ones are not.
            let promise: Promise = ctx.eval("Promise.resolve().then(() => 2)").unwrap();
            assert_eq!(promise.into_future::<i32>().await.unwrap(), 2);
            assert!(!ctx.has_exception());
        }),
    )
    .await
    .expect("a promise future hung");
});

async_test_case!(promise_futures_unchanged_without_interrupt => (rt, ctx) {
    let script = "globalThis.ok = Promise.resolve().then(() => 42);
         globalThis.err = Promise.resolve().then(() => { throw 7; });
         queueMicrotask(() => { throw 2; });
         queueMicrotask(() => { try { throw 3; } catch {} });
         Promise.reject(1).then(() => {}).catch(() => {});";

    // `async_with` driver.
    tokio::time::timeout(
        TIMEOUT,
        ctx.async_with(async |ctx| {
            ctx.eval::<(), _>(script).unwrap();
            let ok: Promise = ctx.globals().get("ok").unwrap();
            assert_eq!(ok.into_future::<i32>().await.unwrap(), 42);
            let err: Promise = ctx.globals().get("err").unwrap();
            assert!(matches!(err.into_future::<()>().await, Err(Error::Exception)));
            assert_eq!(ctx.catch().as_int(), Some(7));
            assert!(!ctx.has_exception());
            assert_eq!(ctx.take_pending_job_error().unwrap().as_int(), Some(2));
        }),
    )
    .await
    .expect("a promise future hung");

    // `idle`, then a later `async_with`.
    ctx.with(|ctx| ctx.eval::<(), _>(script).unwrap()).await;
    tokio::time::timeout(TIMEOUT, rt.idle())
        .await
        .expect("idle hung");
    tokio::time::timeout(
        TIMEOUT,
        ctx.async_with(async |ctx| {
            assert!(!ctx.has_exception());
            let ok: Promise = ctx.globals().get("ok").unwrap();
            assert_eq!(ok.into_future::<i32>().await.unwrap(), 42);
            let err: Promise = ctx.globals().get("err").unwrap();
            assert!(matches!(err.into_future::<()>().await, Err(Error::Exception)));
            assert_eq!(ctx.catch().as_int(), Some(7));
            assert_eq!(ctx.take_pending_job_error().unwrap().as_int(), Some(2));
        }),
    )
    .await
    .expect("a promise future hung");
});

async_test_case!(handler_panic_still_records_interrupt => (rt, ctx) {
    let armed = one_shot_interrupt(&rt).await;
    rt.set_job_error_handler(Some(Box::new(|_, _| panic!("handler boom"))))
        .await;
    ctx.with(|ctx| {
        ctx.eval::<(), _>(script("")).unwrap();
        armed.store(true, SeqCst);
    })
    .await;

    let res = tokio::time::timeout(TIMEOUT, AssertUnwindSafe(rt.idle()).catch_unwind())
        .await
        .expect("idle hung");
    let payload = res.expect_err("the handler panic should propagate out of idle");
    assert_eq!(*payload.downcast_ref::<&str>().unwrap(), "handler boom");
    assert!(!armed.load(SeqCst));
    rt.set_job_error_handler(None).await;

    tokio::time::timeout(
        TIMEOUT,
        ctx.async_with(async |ctx| {
            let promise: Promise = ctx.globals().get("p").unwrap();
            assert_observes_interrupt(&ctx, promise.into_future()).await;
        }),
    )
    .await
    .expect("the promise future hung on an interrupted job");
});
