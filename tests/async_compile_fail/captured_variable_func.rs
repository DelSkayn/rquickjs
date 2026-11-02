use rquickjs::{AsyncContext, AsyncRuntime};

pub async fn test() {
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    let mut var = 1u32;
    let var_ref = &mut var;
    ctx.async_with(async |ctx| {
        ctx.spawn(async move {
            *var_ref += 1;
        })
    })
    .await
}

fn main() {}
