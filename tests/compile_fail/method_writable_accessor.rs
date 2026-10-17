use rquickjs::{class::Trace, JsLifetime};

#[derive(Trace, JsLifetime, Default)]
#[rquickjs::class]
struct Test {
    #[qjs(skip_trace)]
    value: u32,
}

#[rquickjs::methods]
impl Test {
    #[qjs(get, writable)]
    fn value(&self) -> u32 {
        self.value
    }
}

fn main() {}
