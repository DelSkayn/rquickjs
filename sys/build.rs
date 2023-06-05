use std::{
    env, fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use log::info;

fn main() {
    #[cfg(feature = "logging")]
    pretty_env_logger::init();

    println!("cargo:rerun-if-changed=build.rs");

    info!("Begin quickjs compilation");
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

    for feature in &features {
        println!("cargo:rerun-if-env-changed={}", feature_to_cargo(feature));
    }

    let src_dir = Path::new("c-src");
    let header_dir = Path::new("c-include");
    let quickjs_header_dir = header_dir.join("quickjs");
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

    let source_files = vec![
        "libregexp.c",
        "libunicode.c",
        "cutils.c",
        "libbf.c",
        "anode-ext.c",
    ];
    let split_source_dir_1 = src_dir.join("core");
    let split_source_dir_2 = split_source_dir_1.join("builtins");
    // Include every .c file in the two split source dirs
    let split_source_files = split_source_dir_1
        .read_dir()
        .unwrap()
        .filter_map(|p| {
            let p = p.unwrap();
            let path = p.path();
            if path.extension().map(|e| e == "c").unwrap_or(false) {
                Some(path)
            } else {
                None
            }
        })
        .chain(split_source_dir_2.read_dir().unwrap().filter_map(|p| {
            let p = p.unwrap();
            let path = p.path();
            if path.extension().map(|e| e == "c").unwrap_or(false) {
                Some(path)
            } else {
                None
            }
        }));
    let split_source_files = split_source_files
        .map(|x| x.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    info!("Found split source files: {:?}", split_source_files);

    // rerun if anything in c-src or c-include changes
    for file in src_dir.read_dir().expect("Unable to read c-src") {
        let file = file.expect("Unable to read file");
        println!("cargo:rerun-if-changed={}", file.path().display());
    }
    for file in header_dir.read_dir().expect("Unable to read c-include") {
        let file = file.expect("Unable to read file");
        println!("cargo:rerun-if-changed={}", file.path().display());
    }

    let mut patch_files = vec![
    //     "check_stack_overflow.patch",
    //     "infinity_handling.patch",
    //     "atomic_new_class_id.patch",
    ];

    let mut defines = vec![
        ("_GNU_SOURCE".into(), None),
        ("CONFIG_VERSION".into(), Some("\"2020-01-19\"")),
        ("CONFIG_BIGNUM".into(), None),
    ];

    if cfg!(feature = "box64") {
        defines.push(("CONFIG_BOX64".into(), None));
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

    let include_dir = [src_dir, header_dir, &quickjs_header_dir];

    info!("Finished configuration");

    // generating bindings
    info!("Generate bindings");
    bindgen(
        out_dir.join("bindings.rs"),
        split_source_dir_1.join("quickjs-internals.h"),
        &defines,
        include_dir,
    );

    info!("Begin compiler setup");
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
        builder.file(src_dir.join(src));
    }
    for src in &split_source_files {
        builder.file(src);
    }

    builder.include(header_dir);
    builder.include(quickjs_header_dir);
    builder.include(src_dir);

    info!("Begin compiling");
    builder.compile("quickjs");
}

fn feature_to_cargo(name: impl AsRef<str>) -> String {
    format!("CARGO_FEATURE_{}", feature_to_define(name))
}

fn feature_to_define(name: impl AsRef<str>) -> String {
    name.as_ref().to_uppercase().replace('-', "_")
}

fn patch<D: AsRef<Path>, P: AsRef<Path>>(out_dir: D, patch: P) {
    let mut child = Command::new("patch")
        .arg("-p1")
        .stdin(Stdio::piped())
        .current_dir(out_dir)
        .spawn()
        .expect("Unable to execute patch, you may need to install it: {}");
    println!("Appliyng patch {}", patch.as_ref().display());
    {
        let patch = fs::read(patch).expect("Unable to read patch");

        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(&patch).expect("Unable to apply patch");
    }

    child.wait_with_output().expect("Unable to apply patch");
}

#[cfg(not(feature = "bindgen"))]
#[allow(unused)] // until the todo!() is implemented
fn bindgen<'a, D, H, X, K, V>(out_dir: D, _header_file: H, _defines: X)
where
    D: AsRef<Path>,
    H: AsRef<Path>,
    X: IntoIterator<Item = &'a (K, Option<V>)>,
    K: AsRef<str> + 'a,
    V: AsRef<str> + 'a,
{
    todo!("Reimplementing the bindings without bindgen is not supported yet.");

    let target = env::var("TARGET").unwrap();

    let bindings_file = out_dir.as_ref().join("bindings.rs");

    fs::write(
        &bindings_file,
        format!(
            r#"macro_rules! bindings_env {{
                ("TARGET") => {{ "{}" }};
            }}"#,
            target
        ),
    )
    .unwrap();
}

#[cfg(feature = "bindgen")]
fn bindgen<'a, D, H, X, K, V>(
    out_file: D,
    header_file: H,
    defines: X,
    includes: impl IntoIterator<Item = impl AsRef<Path>>,
) where
    D: AsRef<Path>,
    H: AsRef<Path>,
    X: IntoIterator<Item = &'a (K, Option<V>)>,
    K: AsRef<str> + 'a,
    V: AsRef<str> + 'a,
{
    let target = env::var("TARGET").unwrap();
    // let out_dir = out_dir.as_ref();
    let header_file = header_file.as_ref();

    let mut cflags = vec![format!("--target={}", target)];

    for (name, value) in defines {
        cflags.push(if let Some(value) = value {
            format!("-D{}={}", name.as_ref(), value.as_ref())
        } else {
            format!("-D{}", name.as_ref())
        });
    }

    for include in includes {
        cflags.push(format!("-I{}", include.as_ref().display()));
    }

    let bindings = bindgen_rs::Builder::default()
        .detect_include_paths(true)
        .clang_arg("-xc")
        .clang_arg("-v")
        .clang_args(cflags)
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

    // let bindings_file = out_dir.join("bindings.rs");
    let bindings_file = out_file.as_ref();
    println!("cargo:rerun-if-changed={}", header_file.display());

    bindings
        .write_to_file(bindings_file)
        .expect("Couldn't write bindings");

    // Special case to support bundled bindings
    if env::var("CARGO_FEATURE_UPDATE_BINDINGS").is_ok() {
        let dest_dir = Path::new("src").join("bindings");
        fs::create_dir_all(&dest_dir).unwrap();

        let dest_file = format!("{}.rs", target);
        fs::copy(bindings_file, dest_dir.join(&dest_file)).unwrap();
    }
}
