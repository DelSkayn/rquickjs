// TODO trybuild will attempt to execute rustc and thus can't be used in Wasm
// TODO figure out another way to run these tests in wasm32-wasip1

#[cfg(target_arch = "wasm32")]
#[path = "macros/pass_class.rs"]
pub mod pass_class;

#[cfg(target_arch = "wasm32")]
#[path = "macros/pass_method.rs"]
pub mod pass_method;

#[cfg(target_arch = "wasm32")]
#[path = "macros/pass_module.rs"]
pub mod pass_module;

#[cfg(target_arch = "wasm32")]
#[path = "macros/pass_trace.rs"]
pub mod pass_trace;

#[cfg(feature = "macro")]
mod macro_tests {
    #[cfg(target_arch = "wasm32")]
    use crate::{pass_class, pass_method, pass_module, pass_trace};

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn macros() {
        let t = trybuild::TestCases::new();
        t.pass("tests/macros/pass_*.rs");

        #[cfg(feature = "compile-tests")]
        t.compile_fail("tests/compile_fail/*.rs");

        #[cfg(all(feature = "futures", feature = "compile-tests"))]
        t.compile_fail("tests/async_compile_fail/*.rs");

        #[cfg(all(feature = "futures", feature = "parallel", feature = "compile-tests"))]
        t.compile_fail("tests/async_parallel_compile_fail/*.rs");
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn pass_class() {
        pass_class::main();
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn pass_method() {
        pass_method::main();
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn pass_module() {
        pass_module::main();
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn pass_trace() {
        pass_trace::main();
    }
}
