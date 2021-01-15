#[rquickjs::bind(module, public)]
#[quickjs(bare)]
#[allow(non_upper_case_globals)]
pub mod native_module {
    pub const n: i32 = 123;
    pub const s: &str = "abc";
    pub fn f(a: f64, b: f64) -> f64 {
        (a + b) * 0.5
    }
}
