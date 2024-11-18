use rquickjs::{Runtime, Context};

pub fn main() {
    let runtime = Runtime::new().unwrap();
    let context = Context::base(&runtime).unwrap();
}
