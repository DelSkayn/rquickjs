#[cfg(all(feature = "compile-tests", feature = "macro2"))]
#[test]
fn compile_test() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail_macro2/*.rs");
}
