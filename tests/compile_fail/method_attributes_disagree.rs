use rquickjs::{class::Trace, JsLifetime};

#[derive(Trace, JsLifetime, Default)]
#[rquickjs::class]
struct Test {
    #[qjs(skip_trace)]
    value: u32,
}

#[rquickjs::methods]
impl Test {
    #[qjs(get, rename = "value", enumerable)]
    fn get_value(&self) -> u32 {
        self.value
    }

    #[qjs(set, rename = "value", enumerable = false)]
    fn set_value(&mut self, value: u32) {
        self.value = value
    }
}

fn main() {}
