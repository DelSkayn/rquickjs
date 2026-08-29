use rquickjs_jit::abi::{AbiInfo, ABI_MAJOR, ABI_MINOR};

const BUNDLED_TARGETS: [&str; 9] = [
    "x86_64-unknown-linux-gnu.rs",
    "aarch64-unknown-linux-gnu.rs",
    "x86_64-unknown-linux-musl.rs",
    "aarch64-unknown-linux-musl.rs",
    "x86_64-apple-darwin.rs",
    "aarch64-apple-darwin.rs",
    "x86_64-pc-windows-gnu.rs",
    "x86_64-pc-windows-msvc.rs",
    "aarch64-pc-windows-msvc.rs",
];

fn jit_declarations(source: &str) -> String {
    let lines: Vec<_> = source.lines().collect();
    assert!(lines.iter().any(|line| line.starts_with("pub type size_t")));
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.starts_with("pub const __JS_ATOM_NULL"))
            .count(),
        1
    );
    let first_struct = lines
        .iter()
        .position(|line| line.starts_with("pub struct JSJitFunctionId"))
        .expect("JSJitFunctionId declaration");
    let atoms = lines
        .iter()
        .position(|line| line.starts_with("pub const __JS_ATOM_NULL"))
        .expect("atom declarations");
    let mut normalized = String::from("pub type size_t = NORMALIZED;\n");
    for line in &lines {
        if line.starts_with("pub const QJSJIT_ABI_") {
            normalized.push_str(line);
            normalized.push('\n');
        }
    }
    for line in &lines[first_struct - 2..atoms] {
        normalized.push_str(line);
        normalized.push('\n');
    }
    normalized
}

fn bundled_binding(target: &str) -> String {
    let binding_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../sys/src/bindings");
    std::fs::read_to_string(binding_dir.join(target)).unwrap()
}

#[test]
#[allow(clippy::absurd_extreme_comparisons)]
fn linked_abi_matches_rust_contract() {
    let info = AbiInfo::linked().expect("ABI info");
    assert_eq!(info.major(), ABI_MAJOR);
    assert!(info.minor() >= ABI_MINOR);
    assert_eq!(info.pointer_width(), usize::BITS as u8);
    assert_eq!(info.little_endian(), cfg!(target_endian = "little"));
}

#[test]
#[cfg(feature = "test-support")]
fn backend_is_detached_before_runtime_drop() {
    let events = rquickjs_jit::test_support::record_lifecycle();
    {
        let _runtime = events.runtime();
    }
    assert_eq!(
        events.take(),
        ["attach", "detach", "backend_drop", "runtime_drop"]
    );
}

#[test]
#[cfg(feature = "test-support")]
fn cloned_runtime_outlives_the_detached_backend() {
    let events = rquickjs_jit::test_support::record_lifecycle();
    let runtime = events.runtime();
    let runtime_clone = runtime.runtime().clone();
    drop(runtime);

    assert_eq!(events.snapshot(), ["attach", "detach", "backend_drop"]);
    drop(runtime_clone);
    assert_eq!(
        events.take(),
        ["attach", "detach", "backend_drop", "runtime_drop"]
    );
}

#[test]
#[cfg(feature = "test-support")]
fn cloned_context_outlives_the_detached_backend() {
    let events = rquickjs_jit::test_support::record_lifecycle();
    let runtime = events.runtime();
    let context = rquickjs::Context::full(runtime.runtime()).unwrap();
    drop(runtime);

    assert_eq!(events.snapshot(), ["attach", "detach", "backend_drop"]);
    drop(context);
    assert_eq!(
        events.take(),
        ["attach", "detach", "backend_drop", "runtime_drop"]
    );
}

#[test]
#[cfg(feature = "test-support")]
fn duplicate_attachment_does_not_replace_the_first_backend() {
    assert!(rquickjs_jit::test_support::duplicate_attachment_is_rejected());
}

#[test]
#[cfg(feature = "test-support")]
fn every_abi_mismatch_is_rejected_before_backend_storage() {
    use rquickjs_jit::test_support::AbiMismatchFixture;

    for mismatch in AbiMismatchFixture::ALL {
        assert!(
            rquickjs_jit::test_support::mismatch_is_rejected_before_attach(mismatch),
            "fixture was accepted: {mismatch:?}"
        );
    }
}

#[test]
fn bundled_targets_share_jit_declarations() {
    let reference = bundled_binding(BUNDLED_TARGETS[0]);
    let reference = jit_declarations(&reference);
    for target in BUNDLED_TARGETS.iter().skip(1) {
        assert_eq!(
            jit_declarations(&bundled_binding(target)),
            reference,
            "{target}"
        );
    }
}

#[test]
#[cfg(all(feature = "test-support", feature = "bindgen"))]
fn bundled_targets_match_fresh_bindgen_output() {
    let generated = rquickjs_jit::test_support::fresh_bindgen_bindings()
        .expect("test must receive fresh bindgen output");
    let generated = jit_declarations(generated);
    for target in BUNDLED_TARGETS {
        assert_eq!(
            jit_declarations(&bundled_binding(target)),
            generated,
            "{target}"
        );
    }
}
