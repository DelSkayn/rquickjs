//! Async polling benchmarks

use rquickjs::{AsyncContext, AsyncRuntime};
use std::time::Instant;

async fn bench_spawned_futures(n: usize) {
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    let start = Instant::now();

    ctx.with(|ctx| {
        for _ in 0..n {
            ctx.spawn(async {});
        }
    })
    .await;

    rt.idle().await;

    let elapsed = start.elapsed();
    println!(
        "spawn {} futures: {:?} ({:.2} M ops/sec)",
        n,
        elapsed,
        n as f64 / elapsed.as_secs_f64() / 1_000_000.0
    );
}

async fn bench_js_promises(n: usize) {
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    let start = Instant::now();

    ctx.async_with(async |ctx| {
        let code = format!(
            r#"
            (async function run() {{
                let count = 0;
                const promises = [];
                for (let i = 0; i < {n}; i++) {{
                    promises.push(Promise.resolve(i).then(v => count++));
                }}
                await Promise.all(promises);
                return count;
            }})()
        "#
        );
        ctx.eval::<rquickjs::Promise, _>(code)
            .unwrap()
            .into_future::<i32>()
            .await
            .unwrap();
    })
    .await;

    let elapsed = start.elapsed();
    println!(
        "js promises {}: {:?} ({:.2} M ops/sec)",
        n,
        elapsed,
        n as f64 / elapsed.as_secs_f64() / 1_000_000.0
    );
}

async fn bench_chained_promises(depth: usize) {
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    let start = Instant::now();

    ctx.async_with(async |ctx| {
        let code = format!(
            r#"
            (async function run() {{
                let p = Promise.resolve(0);
                for (let i = 0; i < {depth}; i++) {{
                    p = p.then(v => v + 1);
                }}
                return await p;
            }})()
        "#
        );
        ctx.eval::<rquickjs::Promise, _>(code)
            .unwrap()
            .into_future::<i32>()
            .await
            .unwrap();
    })
    .await;

    let elapsed = start.elapsed();
    println!(
        "chained promises {}: {:?} ({:.2} M ops/sec)",
        depth,
        elapsed,
        depth as f64 / elapsed.as_secs_f64() / 1_000_000.0
    );
}

async fn bench_concurrent_spawns(n: usize) {
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    let start = Instant::now();

    ctx.with(|ctx| {
        for _ in 0..100 {
            let ctx2 = ctx.clone();
            ctx.spawn(async move {
                for _ in 0..n / 100 {
                    ctx2.spawn(async {});
                }
            });
        }
    })
    .await;

    rt.idle().await;

    let elapsed = start.elapsed();
    println!(
        "concurrent spawns {}: {:?} ({:.2} M ops/sec)",
        n,
        elapsed,
        n as f64 / elapsed.as_secs_f64() / 1_000_000.0
    );
}

fn main() {
    futures::executor::block_on(async {
        println!("=== Async Benchmarks ===\n");

        bench_spawned_futures(100_000).await;
        bench_spawned_futures(1_000_000).await;

        bench_js_promises(10_000).await;
        bench_js_promises(100_000).await;

        bench_chained_promises(10_000).await;
        bench_chained_promises(100_000).await;

        bench_concurrent_spawns(100_000).await;
        bench_concurrent_spawns(1_000_000).await;
    });
}
