use std::{env, fs, path::Path};

use diffy::{apply, Patch};
use newline_converter::dos2unix;

fn main() {
    #[cfg(feature = "logging")]
    pretty_env_logger::init();

    let features = [
        "exports",
        "bindgen",
        "update-bindings",
        "dump-bytecode",
        "dump-gc",
        "dump-gc-free",
        "dump-free",
        "dump-leaks",
        "dump-mem",
        "dump-objects",
        "dump-atoms",
        "dump-shapes",
        "dump-module-resolve",
        "dump-promise",
        "dump-read-object",
    ];

    println!("cargo:rerun-if-changed=build.rs");
    for feature in &features {
        println!("cargo:rerun-if-env-changed={}", feature_to_cargo(feature));
    }

    let src_dir = Path::new("quickjs");
    let patches_dir = Path::new("patches");

    let out_dir = env::var("OUT_DIR").expect("No OUT_DIR env var is set by cargo");
    let out_dir = Path::new(&out_dir);

    let header_files = [
        "libbf.h",
        "libregexp-opcode.h",
        "libregexp.h",
        "libunicode-table.h",
        "libunicode.h",
        "list.h",
        "quickjs-atom.h",
        "quickjs-opcode.h",
        "quickjs.h",
        "cutils.h",
    ];

    let source_files = [
        "libregexp.c",
        "libunicode.c",
        "cutils.c",
        "quickjs.c",
        "libbf.c",
    ];

    let mut patch_files = vec![
        "error_column_number.patch",
        "get_function_proto.patch",
        "check_stack_overflow.patch",
        "infinity_handling.patch",
        "atomic_new_class_id.patch",
        "dynamic_import_sync.patch",
    ];

    let mut defines = vec![
        ("_GNU_SOURCE".into(), None),
        ("CONFIG_VERSION".into(), Some("\"2020-01-19\"")),
        ("CONFIG_BIGNUM".into(), None),
    ];

    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows"
        && env::var("CARGO_CFG_TARGET_ENV").unwrap() == "msvc"
    {
        patch_files.push("basic_msvc_compat.patch");
    }

    if env::var("CARGO_FEATURE_EXPORTS").is_ok() {
        patch_files.push("read_module_exports.patch");
        defines.push(("CONFIG_MODULE_EXPORTS".into(), None));
    }

    for feature in &features {
        if feature.starts_with("dump-") && env::var(feature_to_cargo(feature)).is_ok() {
            defines.push((feature_to_define(feature), None));
        }
    }

    for file in source_files.iter().chain(header_files.iter()) {
        fs::copy(src_dir.join(file), out_dir.join(file)).expect("Unable to copy source");
    }
    fs::copy("quickjs.bind.h", out_dir.join("quickjs.bind.h")).expect("Unable to copy source");

    // applying patches
    for file in &patch_files {
        patch(out_dir, patches_dir.join(file));
    }

    // generating bindings
    bindgen(out_dir, out_dir.join("quickjs.bind.h"), &defines);

    let mut builder = cc::Build::new();
    builder
        .extra_warnings(false)
        //.flag("-Wno-array-bounds")
        //.flag("-Wno-format-truncation")
        ;

    for (name, value) in &defines {
        builder.define(name, *value);
    }

    for src in &source_files {
        builder.file(out_dir.join(src));
    }

    builder.compile("libquickjs.a");
}

fn feature_to_cargo(name: impl AsRef<str>) -> String {
    format!("CARGO_FEATURE_{}", feature_to_define(name))
}

fn feature_to_define(name: impl AsRef<str>) -> String {
    name.as_ref().to_uppercase().replace('-', "_")
}

fn patch<D: AsRef<Path>, P: AsRef<Path>>(out_dir: D, patch: P) {
    struct Patches<'a>(&'a str);

    // A backtracking attempt to find a valid diff when there are multiple patches in one file (also known as patchset)
    impl<'a> Iterator for Patches<'a> {
        type Item = Patch<'a, str>;
        fn next(&mut self) -> Option<Self::Item> {
            let mut range = self.0.len();

            loop {
                let input = if let Some(input) = self.0.get(..range) {
                    input
                } else {
                    range -= 1;
                    continue;
                };
                match Patch::from_str(input) {
                    Ok(x) if x.hunks().is_empty() => break None,
                    Err(_) if range < 1 => break None,
                    Ok(x) => {
                        self.0 = &self.0[range..];
                        break Some(x);
                    }
                    Err(_) => range -= 1,
                }
            }
        }
    }

    println!("Appliyng patch {}", patch.as_ref().display());
    {
        let out_dir = out_dir.as_ref();
        let patch = fs::read_to_string(patch).expect("Unable to read patch");
        let patch = dos2unix(&patch);
        for patch in Patches(&patch) {
            let original = patch
                .original()
                .and_then(|x| x.split_once('/').map(|(_, b)| b))
                .or(patch.original())
                .expect("Cannot find original file name");
            let modified = patch
                .modified()
                .and_then(|x| x.split_once('/').map(|(_, b)| b))
                .or(patch.modified())
                .expect("Cannot find modified file name");

            let original_path = out_dir.join(original);
            let modified_path = out_dir.join(modified);

            let original = if !original_path.exists() && !modified_path.exists() {
                String::new()
            } else {
                fs::read_to_string(original_path).expect("Unable to read original file")
            };
            let original = dos2unix(&original);
            match apply(&original, &patch) {
                Ok(patched) => {
                    if let Some(parent) = modified_path.parent() {
                        if !parent.exists() {
                            fs::create_dir_all(parent).unwrap();
                        }
                    }
                    fs::write(modified_path, patched)
                                    .expect("Unable to write the patched content")
                },
                Err(e) => eprintln!("Unable to write the patched content: {}", e),
            }
        }
    }
}

#[cfg(not(feature = "bindgen"))]
fn bindgen<'a, D, H, X, K, V>(out_dir: D, _header_file: H, _defines: X)
where
    D: AsRef<Path>,
    H: AsRef<Path>,
    X: IntoIterator<Item = &'a (K, Option<V>)>,
    K: AsRef<str> + 'a,
    V: AsRef<str> + 'a,
{
    let target = env::var("TARGET").unwrap();

    if !Path::new("./")
        .join("src")
        .join("bindings")
        .join(format!("{}.rs", target))
        .canonicalize()
        .map(|x| x.exists())
        .unwrap_or(false)
    {
        println!(
            "cargo:warning=rquickjs probably doesn't ship bindings for platform `{}`. try the `bindgen` feature instead.",
            target
        );
    }

    let bindings_file = out_dir.as_ref().join("bindings.rs");

    fs::write(
        bindings_file,
        format!(
            r#"macro_rules! bindings_env {{
                ("TARGET") => {{ "{target}" }};
            }}"#
        ),
    )
    .unwrap();
}

#[cfg(feature = "bindgen")]
fn bindgen<'a, D, H, X, K, V>(out_dir: D, header_file: H, defines: X)
where
    D: AsRef<Path>,
    H: AsRef<Path>,
    X: IntoIterator<Item = &'a (K, Option<V>)>,
    K: AsRef<str> + 'a,
    V: AsRef<str> + 'a,
{
    let target = env::var("TARGET").unwrap();
    let out_dir = out_dir.as_ref();
    let header_file = header_file.as_ref();

    let mut cflags = vec![format!("--target={}", target)];

    //format!("-I{}", out_dir.parent().display()),

    for (name, value) in defines {
        cflags.push(if let Some(value) = value {
            format!("-D{}={}", name.as_ref(), value.as_ref())
        } else {
            format!("-D{}", name.as_ref())
        });
    }

    let bindings = bindgen_rs::Builder::default()
        .detect_include_paths(true)
        .clang_arg("-xc")
        .clang_arg("-v")
        .clang_args(cflags)
        .size_t_is_usize(false)
        .header(header_file.display().to_string())
        .allowlist_type("JS.*")
        .allowlist_function("js.*")
        .allowlist_function("JS.*")
        .allowlist_function("__JS.*")
        .allowlist_var("JS.*")
        .opaque_type("FILE")
        .blocklist_type("FILE")
        .blocklist_function("JS_DumpMemoryUsage")
        .generate()
        .expect("Unable to generate bindings");

    let bindings_file = out_dir.join("bindings.rs");

    bindings
        .write_to_file(&bindings_file)
        .expect("Couldn't write bindings");

    // Special case to support bundled bindings
    if env::var("CARGO_FEATURE_UPDATE_BINDINGS").is_ok() {
        let dest_dir = Path::new("src").join("bindings");
        fs::create_dir_all(&dest_dir).unwrap();

        let dest_file = format!("{}.rs", target);
        fs::copy(&bindings_file, dest_dir.join(dest_file)).unwrap();
    }
}
