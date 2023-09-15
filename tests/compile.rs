#[cfg(feature = "macro")]
#[test]
fn macros() {
    let t = trybuild::TestCases::new();
    t.pass("tests/macros/pass_*.rs");
    #[cfg(feature = "compile_tests")]
    t.compile_fail("tests/compile_fail/*.rs");
    #[cfg(all(feature = "futures", feature = "compile_tests"))]
    t.compile_fail("tests/async_compile_fail/*.rs");
    #[cfg(all(feature = "futures", feature = "parallel", feature = "compile_tests"))]
    t.compile_fail("tests/async_parallel_compile_fail/*.rs");
}
