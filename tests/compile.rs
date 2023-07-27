#[cfg(feature = "macro")]
#[test]
fn macros() {
    let t = trybuild::TestCases::new();
    t.pass("tests/macros/pass_*.rs");
}
