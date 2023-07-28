#[cfg(feature = "macro")]
#[test]
fn macros() {
    let t = trybuild::TestCases::new();
    t.pass("tests/macros/pass_*.rs");
    t.compile_fail("tests/compile_fail/*.rs");
    #[cfg(feature = "futures")]
    t.compile_fail("tests/async_compile_fail/*.rs");
    #[cfg(all(feature = "futures", feature = "parallel"))]
    t.compile_fail("tests/async_parallel_compile_fail/*.rs");
}
