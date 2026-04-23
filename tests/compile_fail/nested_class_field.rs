use rquickjs::{class::Trace, JsLifetime};

#[derive(Trace, JsLifetime, Default, Clone)]
#[rquickjs::class]
struct Outer {
    #[qjs(get, set)]
    inner: Inner,
}

#[derive(Trace, JsLifetime, Default, Clone)]
#[rquickjs::class]
struct Inner {
    #[qjs(get, set)]
    value: String,
}

fn main() {}
