use std::rc::Rc;

use rquickjs::{async_with, AsyncContext, AsyncRuntime};

pub async fn test() {
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    let fut = {
        let mut var = Rc::new(1);
        let var_c = var.clone();
        ctx.async_with(|_ctx| {
            // you should not be able to move non send types into the closure.
            assert_eq!(*var_c, 1);
        })
    };
    fut.await;
}

fn main() {}
