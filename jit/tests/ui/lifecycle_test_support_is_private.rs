use rquickjs_jit::{test_support, Runtime};

fn main() {
    let runtime = Runtime::new().unwrap();
    runtime.set_jit_runtime_drop_probe(|| {});
    let _ = test_support::record_lifecycle();
}
