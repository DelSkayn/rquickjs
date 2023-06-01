use rquickjs::{async_with, AsyncContext, AsyncRuntime};

pub async fn test() {
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    let fut = {
        let mut var = 1u32;
        let var_ref = &mut var;
        async_with!(ctx => |_ctx|{
            *var_ref += 1;
        })
    };
    fut.await;
}

fn main() {}
