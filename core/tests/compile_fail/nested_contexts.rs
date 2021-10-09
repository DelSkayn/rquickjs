use rquickjs::{Context, Function, Runtime, String};

fn main() {
    let rt = Runtime::new().unwrap();
    let ctx_1 = Context::full(&rt).unwrap();
    let ctx_2 = Context::full(&rt).unwrap();
    ctx_1.with(|ctx_1| {
        let val: String = ctx_1.eval("'foo'").unwrap();
        ctx_2.with(|ctx_2| {
            let f: Function = ctx_2.eval("x => x + 'b'").unwrap();
            f.call::<_, ()>(val).unwrap();
        })
    })
}
