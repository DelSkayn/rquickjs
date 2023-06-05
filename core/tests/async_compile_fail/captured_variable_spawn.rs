use rquickjs::{async_with, prelude::*, AsyncContext, AsyncRuntime};

pub async fn test() {
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    let mut var = 1u32;
    let var_ref = &mut var;
    async_with!(ctx => |ctx|{
        let func = Func::from(MutFn::from(move ||{
            *var_ref += 1;
        }));
        ctx.globals().set("t",func).unwrap();
    })
    .await
}

fn main() {}
