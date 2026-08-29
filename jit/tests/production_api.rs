#[test]
#[cfg(not(feature = "test-support"))]
fn lifecycle_test_support_is_not_in_the_production_api() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/lifecycle_test_support_is_private.rs");
}
