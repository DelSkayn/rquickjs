#![allow(unused_must_use)]

//! Promise futures observing interrupted jobs across the async job drains.
//!
//! Each test is bounded by a timeout, so a future that misses an interrupted
//! job fails instead of hanging. Interrupt handlers disarm themselves after
//! the first interrupt and are armed only once the looping job is queued.

use std::sync::{
    atomic::{AtomicBool, Ordering::SeqCst},
    Arc,
};

use futures_rs::poll;
use tokio::sync::oneshot;

use super::test_util::{spawn_drive, TIMEOUT};
use crate::{promise::PromiseFuture, AsyncContext, AsyncRuntime, Ctx, Error, Module, Promise};

const LOOPING_JOB: &str = "Promise.resolve().then(() => { for (;;) {} })";

struct Case {
    later: &'static str,
    host_catch: bool,
}

const LATER_THROW: Case = Case {
    later: "queueMicrotask(() => { throw 2; });",
    host_catch: false,
};

const LATER_TRY_AWAIT: Case = Case {
    later: "(async () => { try { await Promise.reject(1); } catch {} })();",
    host_catch: false,
};

const HOST_CATCH: Case = Case {
    later: "",
    host_catch: true,
};

async fn one_shot_interrupt(rt: &AsyncRuntime) -> Arc<AtomicBool> {
    let armed = Arc::new(AtomicBool::new(false));
    let handler_armed = armed.clone();
    rt.set_interrupt_handler(Some(Box::new(move || handler_armed.swap(false, SeqCst))))
        .await;
    armed
}

fn script(later: &str) -> String {
    format!("globalThis.p = {LOOPING_JOB}; {later} p")
}

fn host_throw_and_catch(ctx: &Ctx<'_>) {
    let _ = ctx.eval::<(), _>("throw 1").map_err(|_| ctx.catch());
}

async fn assert_observes_interrupt<'js>(ctx: &Ctx<'js>, future: PromiseFuture<'js, ()>) {
    let res = future.await;
    assert!(matches!(res, Err(Error::Exception)));
    assert!(ctx.catch().is_uncatchable_error());
    assert!(!ctx.has_exception());
    let sum: i32 = ctx.eval("1 + 1").unwrap();
    assert_eq!(sum, 2);
}

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
        }),
    )
    .await
    .expect("the promise future hung on an interrupted job");
}

async fn drive_case(rt: &AsyncRuntime, ctx: &AsyncContext, case: Case) {
    let armed = one_shot_interrupt(rt).await;
    let drive = spawn_drive(rt);

    ctx.async_with(async |ctx| {
        ctx.eval::<(), _>(script(case.later)).unwrap();
        armed.store(true, SeqCst);
        // Wake the drive task so its drain runs the queued jobs.
        ctx.spawn(async move {});
    })
    .await
    .unwrap();

    tokio::time::timeout(TIMEOUT, drive)
        .await
        .expect("the drive task hung")
        .expect("the drive task panicked")
        .expect_err("the drive task should report the interrupted job");
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
    .expect("the promise future hung on an interrupted job")
    .unwrap();
}

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
        }),
    )
    .await
    .expect("the promise future hung on an interrupted job");
}

async_test_case!(async_with_observes_interrupt_despite_later_work => (_rt, _ctx) {
    for case in [LATER_THROW, LATER_TRY_AWAIT, HOST_CATCH] {
        let rt = AsyncRuntime::new().unwrap();
        let ctx = AsyncContext::full(&rt).await.unwrap();
        async_with_case(&rt, &ctx, case).await;
    }
});

async_test_case!(drive_observes_interrupt_despite_later_work => (_rt, _ctx) {
    for case in [LATER_THROW, LATER_TRY_AWAIT, HOST_CATCH] {
        let rt = AsyncRuntime::new().unwrap();
        let ctx = AsyncContext::full(&rt).await.unwrap();
        drive_case(&rt, &ctx, case).await;
    }
});

async_test_case!(idle_observes_interrupt_despite_later_work => (_rt, _ctx) {
    for case in [LATER_THROW, LATER_TRY_AWAIT, HOST_CATCH] {
        let rt = AsyncRuntime::new().unwrap();
        let ctx = AsyncContext::full(&rt).await.unwrap();
        idle_case(&rt, &ctx, case).await;
    }
});

async_test_case!(idle_wakes_spawned_observer => (rt, ctx) {
    let armed = one_shot_interrupt(&rt).await;
    let (tx, rx) = oneshot::channel();
    ctx.with(|ctx| {
        let observer = ctx.clone();
        let armed = armed.clone();
        ctx.spawn(async move {
            let promise: Promise = observer.eval(LOOPING_JOB).unwrap();
            let mut future = promise.into_future::<()>();
            assert!(poll!(&mut future).is_pending());
            armed.store(true, SeqCst);
            assert!(matches!(future.await, Err(Error::Exception)));
            assert!(observer.catch().is_uncatchable_error());
            assert!(!observer.has_exception());
            let _ = tx.send(());
        });
    })
    .await;

    // The observer queues the looping job on its first poll; the interrupt is
    // reported by the drain.
    tokio::time::timeout(TIMEOUT, rt.idle())
        .await
        .expect("idle hung")
        .expect_err("idle should report the interrupted job");
    assert!(!armed.load(SeqCst));

    // The woken observer observes the recorded interrupt.
    tokio::time::timeout(TIMEOUT, rt.idle())
        .await
        .expect("idle hung")
        .unwrap();
    tokio::time::timeout(TIMEOUT, rx)
        .await
        .expect("the spawned promise future hung on an interrupted job")
        .unwrap();
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
    })
    .await;
});

async_test_case!(unobserved_interrupt_fails_one_later_future => (rt, ctx) {
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
            // though its own promise is unrelated (the documented
            // per-context scope).
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

#[tokio::test]
async fn recorded_interrupt_is_scoped_to_the_interrupted_jobs_context() {
    use core::{
        future::Future,
        pin::Pin,
        task::{Context as TaskContext, Poll, Waker},
    };

    let rt = AsyncRuntime::new().unwrap();
    let interrupted = AsyncContext::full(&rt).await.unwrap();
    let other = AsyncContext::full(&rt).await.unwrap();
    let armed = one_shot_interrupt(&rt).await;

    interrupted
        .with(|ctx| {
            ctx.eval::<(), _>(LOOPING_JOB).unwrap();
            armed.store(true, SeqCst);
        })
        .await;
    tokio::time::timeout(TIMEOUT, rt.idle())
        .await
        .expect("idle hung");
    assert!(!armed.load(SeqCst));

    // A promise future in another context does not take the record.
    other
        .async_with(async |ctx| {
            let (promise, _resolve, _reject) = Promise::new(&ctx).unwrap();
            let mut future = promise.into_future::<()>();
            let mut cx = TaskContext::from_waker(Waker::noop());
            assert!(matches!(Pin::new(&mut future).poll(&mut cx), Poll::Pending));
            assert!(!ctx.has_exception());
        })
        .await;

    // In the interrupted job's context the next unsettled future fails.
    interrupted
        .async_with(async |ctx| {
            let promise: Promise = ctx.eval("Promise.resolve().then(() => 1)").unwrap();
            assert!(matches!(
                promise.into_future::<i32>().await,
                Err(Error::Exception)
            ));
            assert!(ctx.catch().is_uncatchable_error());
            assert!(!ctx.has_exception());
        })
        .await;
}

async_test_case!(module_load_after_failed_job => (rt, ctx) {
    ctx.with(|ctx| {
        ctx.eval::<(), _>("queueMicrotask(() => { throw 1; });")
            .unwrap();
    })
    .await;
    tokio::time::timeout(TIMEOUT, rt.idle())
        .await
        .expect("idle hung");

    ctx.with(|ctx| {
        let (_, promise) = Module::declare(ctx.clone(), "m", "export const x = 1;")
            .unwrap()
            .eval()
            .unwrap();
        promise.finish::<()>().unwrap();
    })
    .await;
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
        }),
    )
    .await
    .expect("a promise future hung");
});

async_test_case!(idle_reports_interrupt_and_future_still_observes_it => (rt, ctx) {
    let armed = one_shot_interrupt(&rt).await;
    ctx.with(|ctx| {
        ctx.eval::<(), _>(script("")).unwrap();
        armed.store(true, SeqCst);
    })
    .await;

    tokio::time::timeout(TIMEOUT, rt.idle())
        .await
        .expect("idle hung")
        .expect_err("idle should report the interrupted job");
    assert!(!armed.load(SeqCst));

    tokio::time::timeout(
        TIMEOUT,
        ctx.async_with(async |ctx| {
            let promise: Promise = ctx.globals().get("p").unwrap();
            assert_observes_interrupt(&ctx, promise.into_future()).await;
        }),
    )
    .await
    .expect("the promise future hung on an interrupted job")
    .unwrap();
});
