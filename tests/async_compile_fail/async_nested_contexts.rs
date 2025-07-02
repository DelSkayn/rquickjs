use rquickjs::{async_with, AsyncContext, AsyncRuntime};

pub async fn test() {
    let rt = AsyncRuntime::new().unwrap();
    let ctx_1 = AsyncContext::full(&rt).await.unwrap();
    let ctx_2 = AsyncContext::full(&rt).await.unwrap();
    async_with!(ctx_1 => |ctx_1|{
        async_with!(ctx_2 => |ctx_2|{
            // It is disallowed to use multiple ctx object together from different with closures.
            // Lifetime on ctx should be unique.
            ctx_1.globals().set("t", ctx_2.globals());
        }).await
    })
    .await
}

fn main() {}
