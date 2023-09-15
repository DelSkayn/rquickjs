use rquickjs::{Context, Runtime};

fn main() {
    let rt = Runtime::new().unwrap();
    let ctx_1 = Context::full(&rt).unwrap();
    let ctx_2 = Context::full(&rt).unwrap();
    ctx_1.with(|ctx_1| {
        ctx_2.with(|ctx_2| {
            ctx_1.globals().set("t", ctx_2.globals());
        })
    })
}
