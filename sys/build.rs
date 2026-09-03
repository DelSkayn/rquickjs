#![allow(clippy::uninlined_format_args)]
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{self},
};

// WASI logic lifted from https://github.com/bytecodealliance/javy/blob/61616e1507d2bf896f46dc8d72687273438b58b2/crates/quickjs-wasm-sys/build.rs#L18

const WASI_SDK_VERSION_MAJOR: usize = 24;
const WASI_SDK_VERSION_MINOR: usize = 0;

fn download_wasi_sdk() -> PathBuf {
    let mut wasi_sdk_dir: PathBuf = env::var("OUT_DIR").unwrap().into();
    wasi_sdk_dir.push("wasi-sdk");

    fs::create_dir_all(&wasi_sdk_dir).unwrap();

    let major_version = WASI_SDK_VERSION_MAJOR;
    let minor_version = WASI_SDK_VERSION_MINOR;

    let mut archive_path = wasi_sdk_dir.clone();
    archive_path.push(format!("wasi-sdk-{major_version}-{minor_version}.tar.gz"));

    println!("SDK tar: {archive_path:?}");

    // Download archive if necessary
    if !archive_path.try_exists().unwrap() {
        let file_suffix = match (env::consts::OS, env::consts::ARCH) {
            ("linux", "x86") | ("linux", "x86_64") => "x86_64-linux",
            ("linux", "aarch64") => "arm64-linux",
            ("macos", "x86") | ("macos", "x86_64") => "x86_64-macos",
            ("macos", "aarch64") => "arm64-macos",
            ("windows", "x86") | ("windows", "x86_64") => "x86_64-windows",
            ("windows", "aarch64") => "arm64-windows",
            other => panic!("Unsupported platform tuple {:?}", other),
        };

        let uri = format!("https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-{major_version}/wasi-sdk-{major_version}.{minor_version}-{file_suffix}.tar.gz");

        println!("Downloading WASI SDK archive from {uri} to {archive_path:?}");

        let output = process::Command::new("curl")
            .args([
                "--location",
                "-o",
                archive_path.to_string_lossy().as_ref(),
                uri.as_ref(),
            ])
            .output()
            .expect("failed to download the WASI SDK with curl");
        println!("curl output: {}", String::from_utf8_lossy(&output.stdout));
        println!("curl err: {}", String::from_utf8_lossy(&output.stderr));
        if !output.status.success() {
            panic!(
                "curl WASI SDK failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    let mut test_binary = wasi_sdk_dir.clone();
    test_binary.extend(["bin", "wasm-ld"]);
    // Extract archive if necessary
    if !test_binary.try_exists().unwrap() {
        println!("Extracting WASI SDK archive {archive_path:?}");
        let output = process::Command::new("tar")
            .args([
                "-zxf",
                archive_path.to_string_lossy().as_ref(),
                "--strip-components",
                "1",
            ])
            .current_dir(&wasi_sdk_dir)
            .output()
            .unwrap();
        if !output.status.success() {
            panic!(
                "Unpacking WASI SDK failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    wasi_sdk_dir
}

fn get_wasi_sdk_path() -> PathBuf {
    std::env::var_os("WASI_SDK")
        .map(PathBuf::from)
        .unwrap_or_else(download_wasi_sdk)
}

// sha256 of wasi-sysroot-24.0.tar.gz as published on the wasi-sdk-24 release.
const WASI_SYSROOT_SHA256: &str =
    "35172f7d2799485b15a46b1d87f50a585d915ec662080f005d99153a50888f08";

/// Does `path` look like a wasi-sysroot?
fn is_wasi_sysroot(path: &Path) -> bool {
    path.join("include/wasm32-wasi/wasi/api.h")
        .try_exists()
        .unwrap_or(false)
}

/// Root of the wasi-sysroot used for the `wasm32-unknown-unknown` target,
/// honouring `RQUICKJS_WASM_SYSROOT` (which skips the download entirely).
fn get_wasm_sysroot_path() -> PathBuf {
    let Some(path) = env::var_os("RQUICKJS_WASM_SYSROOT").map(PathBuf::from) else {
        return download_wasi_sysroot();
    };
    assert!(
        is_wasi_sysroot(&path),
        "RQUICKJS_WASM_SYSROOT points at {}, which is not a wasi-sysroot \
         (no include/wasm32-wasi/wasi/api.h)",
        path.display(),
    );
    path
}

/// Download and extract the pinned wasi-sysroot.
///
/// Unlike [`download_wasi_sdk`] this caches under `CARGO_HOME` rather than
/// `OUT_DIR`: the archive is 68MB and `OUT_DIR` is wiped by `cargo clean` and
/// is per-profile, so an `OUT_DIR` cache re-downloads far too often. `OUT_DIR`
/// is still the fallback when `CARGO_HOME` is unset.
fn download_wasi_sysroot() -> PathBuf {
    let major_version = WASI_SDK_VERSION_MAJOR;
    let minor_version = WASI_SDK_VERSION_MINOR;

    let cache_dir: PathBuf = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::var("OUT_DIR").unwrap().into())
        .join("rquickjs-wasi-sysroot");
    fs::create_dir_all(&cache_dir).unwrap();

    let archive_path = cache_dir.join(format!(
        "wasi-sysroot-{major_version}.{minor_version}.tar.gz"
    ));
    let sysroot_dir = cache_dir.join(format!("wasi-sysroot-{major_version}.{minor_version}"));

    // Download archive if necessary
    if !archive_path.try_exists().unwrap() {
        let uri = format!(
            "https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-{major_version}/wasi-sysroot-{major_version}.{minor_version}.tar.gz"
        );
        println!("Downloading wasi-sysroot archive from {uri} to {archive_path:?}");

        // download to a temp path first so an interrupted curl cannot leave a
        // truncated archive that later runs would treat as cached.
        let partial_path = archive_path.with_extension("part");
        let output = process::Command::new("curl")
            .args([
                "--location",
                "--fail",
                "-o",
                partial_path.to_string_lossy().as_ref(),
                uri.as_ref(),
            ])
            .output()
            .expect("failed to download the wasi-sysroot with curl");
        if !output.status.success() {
            panic!(
                "curl wasi-sysroot failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        verify_sha256(&partial_path, WASI_SYSROOT_SHA256);
        fs::rename(&partial_path, &archive_path).unwrap();
    } else {
        verify_sha256(&archive_path, WASI_SYSROOT_SHA256);
    }

    // Extract archive if necessary
    let test_header = sysroot_dir.join("include/wasm32-wasi/wasi/api.h");
    if !test_header.try_exists().unwrap() {
        println!("Extracting wasi-sysroot archive {archive_path:?}");
        fs::create_dir_all(&sysroot_dir).unwrap();
        let output = process::Command::new("tar")
            .args([
                "-zxf",
                archive_path.to_string_lossy().as_ref(),
                "--strip-components",
                "1",
            ])
            .current_dir(&sysroot_dir)
            .output()
            .unwrap();
        if !output.status.success() {
            panic!(
                "Unpacking wasi-sysroot failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    sysroot_dir
}

fn verify_sha256(path: &Path, expected: &str) {
    let output = process::Command::new("sha256sum")
        .arg(path)
        .output()
        .or_else(|_| {
            process::Command::new("shasum")
                .args(["-a", "256"])
                .arg(path)
                .output()
        })
        .expect("failed to run sha256sum/shasum to verify the wasi-sysroot archive");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let actual = stdout.split_whitespace().next().unwrap_or_default();
    if !output.status.success() || actual != expected {
        panic!(
            "wasi-sysroot checksum mismatch for {}: expected {expected}, got {actual}. \
             Delete the file and retry, or set RQUICKJS_WASM_SYSROOT to a trusted sysroot.",
            path.display()
        );
    }
}

/// `wasi/api.h` opens with an `#ifndef __wasi__` / `#error` guard, so it cannot
/// be included from `wasm32-unknown-unknown` as-is. Write a copy with that one
/// `#error` commented out into `OUT_DIR`, and return the include directory
/// holding it so it can be placed ahead of the sysroot on the include path.
///
/// The sysroot itself is left untouched: it is a shared cache, and may well be
/// a system-wide install that `RQUICKJS_WASM_SYSROOT` points at.
///
/// Defining `__wasi__` instead is *not* an option: quickjs's `__wasi__` path
/// forces `stack_limit = 0`, disabling stack-overflow checking entirely.
fn patched_wasi_headers(sysroot: &Path, out_dir: &Path) -> PathBuf {
    const MARKER: &str = "/* #error patched out by rquickjs-sys for wasm32-unknown-unknown */";

    let source = sysroot.join("include/wasm32-wasi/wasi/api.h");
    let contents = fs::read_to_string(&source)
        .unwrap_or_else(|err| panic!("unable to read {}: {err}", source.display()));

    // comment out only the `#error` directly inside the `#ifndef __wasi__`
    // guard, leaving the neighbouring `#ifndef __wasm32__` guard intact.
    let mut in_guard = false;
    let patched = contents
        .lines()
        .map(|line| {
            match line.trim() {
                "#ifndef __wasi__" => in_guard = true,
                "#endif" => in_guard = false,
                trimmed if in_guard && trimmed.starts_with("#error") => return MARKER,
                _ => {}
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n");

    let include_dir = out_dir.join("wasm-include");
    fs::create_dir_all(include_dir.join("wasi")).unwrap();
    fs::write(include_dir.join("wasi/api.h"), patched + "\n").expect("unable to write wasi/api.h");
    include_dir
}

fn main() {
    #[cfg(feature = "logging")]
    pretty_env_logger::init();

    let features = [
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
        "disable-assertions",
    ];

    for feature in &features {
        println!("cargo:rerun-if-env-changed={}", feature_to_cargo(feature));
    }
    println!("cargo:rerun-if-env-changed=CARGO_CFG_SANITIZE");

    let src_dir = Path::new("quickjs");

    let out_dir = env::var("OUT_DIR").expect("No OUT_DIR env var is set by cargo");
    let out_dir = Path::new(&out_dir);

    let header_files = [
        "builtin-array-fromasync.h",
        "builtin-iterator-zip-keyed.h",
        "builtin-iterator-zip.h",
        "cutils.h",
        "dtoa.h",
        "libregexp-opcode.h",
        "libregexp.h",
        "libunicode-table.h",
        "libunicode.h",
        "list.h",
        "quickjs-atom.h",
        "quickjs-opcode.h",
        "quickjs-c-atomics.h",
        "quickjs.h",
    ];

    let source_files = ["libregexp.c", "libunicode.c", "quickjs.c", "dtoa.c"];

    let mut defines: Vec<(String, Option<&str>)> = vec![("_GNU_SOURCE".into(), None)];

    #[cfg(feature = "disable-assertions")]
    defines.push(("NDEBUG".into(), None));

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();

    let mut builder = cc::Build::new();
    builder
        .extra_warnings(false)
        .flag_if_supported("-Wno-implicit-const-int-float-conversion")
        //.flag("-Wno-array-bounds")
        //.flag("-Wno-format-truncation")
        ;

    match env::var("CARGO_CFG_SANITIZE").as_deref() {
        Ok("address") => {
            builder
                .flag("-fsanitize=address")
                .flag("-fno-sanitize-recover=all")
                .flag("-fno-omit-frame-pointer");
        }
        Ok("memory") => {
            builder
                .flag("-fsanitize=memory")
                .flag("-fno-sanitize-recover=all")
                .flag("-fno-omit-frame-pointer");
        }
        Ok("thread") => {
            builder
                .flag("-fsanitize=thread")
                .flag("-fno-sanitize-recover=all")
                .flag("-fno-omit-frame-pointer");
        }
        Ok(x) => println!("cargo:warning=Unsupported sanitize_option: '{x}'"),
        _ => {}
    }

    let mut bindgen_cflags = vec![];

    if target_os == "windows" {
        if target_env == "msvc" {
            env::set_var(
                "CFLAGS",
                "/DWIN32_LEAN_AND_MEAN /std:c11 /experimental:c11atomics",
            );
        } else {
            env::set_var("CFLAGS", "-DWIN32_LEAN_AND_MEAN -std=c11");
        }
    }

    // wasm32-unknown-unknown takes the same emscripten-flavoured config as wasi,
    // but ships no libc: headers come from a wasi-sysroot include tree and the OS
    // tail (clock, stdio, abort) is supplied by `wasm-shim/shim.c`.
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let is_wasm_unknown = target_arch == "wasm32" && target_os == "unknown";

    if target_os == "wasi" || is_wasm_unknown {
        // pretend we're emscripten - there are already ifdefs that match
        // also, wasi doesn't have FE_DOWNWARD or FE_UPWARD
        defines.push(("EMSCRIPTEN".into(), Some("1")));
        defines.push(("FE_DOWNWARD".into(), Some("0")));
        defines.push(("FE_UPWARD".into(), Some("0")));
    }

    let wasm_sysroot = is_wasm_unknown.then(|| {
        println!("cargo:rerun-if-env-changed=RQUICKJS_WASM_SYSROOT");
        let sysroot = get_wasm_sysroot_path();
        // the patched copy of `wasi/api.h` has to shadow the sysroot's own, so
        // its include dir goes first.
        for dir in [
            patched_wasi_headers(&sysroot, out_dir),
            sysroot.join("include/wasm32-wasi"),
        ] {
            let flag = format!("-isystem{}", dir.display());
            builder.flag(&flag);
            bindgen_cflags.push(flag);
        }
        sysroot
    });

    for file in source_files.iter().chain(header_files.iter()) {
        fs::copy(src_dir.join(file), out_dir.join(file))
            .expect("Unable to copy source; try 'git submodule update --init'");
    }
    fs::copy("quickjs.bind.h", out_dir.join("quickjs.bind.h")).expect("Unable to copy source");

    if target_os == "wasi" && !matches!(env::var("RQUICKJS_SYS_NO_WASI_SDK").as_deref(), Ok("1")) {
        let wasi_sdk_path = get_wasi_sdk_path();
        if !wasi_sdk_path.try_exists().unwrap() {
            panic!(
                "wasi-sdk not installed in specified path of {}",
                wasi_sdk_path.display()
            );
        }
        env::set_var("CC", wasi_sdk_path.join("bin/clang").to_str().unwrap());
        env::set_var("AR", wasi_sdk_path.join("bin/ar").to_str().unwrap());
        let sysroot = format!(
            "--sysroot={}",
            wasi_sdk_path.join("share/wasi-sysroot").display()
        );
        env::set_var("CFLAGS", &sysroot);
        bindgen_cflags.push(sysroot);
    }

    // generating bindings
    bindgen(
        out_dir,
        out_dir.join("quickjs.bind.h"),
        &defines,
        bindgen_cflags,
    );

    for (name, value) in &defines {
        builder.define(name, *value);
    }

    for src in &source_files {
        builder.file(out_dir.join(src));
    }

    if is_wasm_unknown {
        // the shim goes into libquickjs.a rather than its own archive: lld
        // resolves an archive to a fixpoint, so every definition here is picked
        // up before wasi-libc is reached and the matching wasi-libc member (and
        // its wasi imports) is never extracted.
        println!("cargo:rerun-if-changed=wasm-shim/shim.c");
        builder.file("wasm-shim/shim.c");
    }

    builder.compile("libquickjs.a");

    // emitted after `compile` so `-lquickjs` precedes `-lc` on the link line.
    if let Some(sysroot) = wasm_sysroot {
        println!(
            "cargo:rustc-link-search=native={}",
            sysroot.join("lib/wasm32-wasi").display()
        );
        println!("cargo:rustc-link-lib=static=c");
    }
}

fn feature_to_cargo(name: impl AsRef<str>) -> String {
    format!("CARGO_FEATURE_{}", feature_to_define(name))
}

fn feature_to_define(name: impl AsRef<str>) -> String {
    name.as_ref().to_uppercase().replace('-', "_")
}

#[cfg(not(feature = "bindgen"))]
fn bindgen<'a, D, H, X, K, V>(out_dir: D, _header_file: H, _defines: X, _add_cflags: Vec<String>)
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
            "cargo:warning=rquickjs probably doesn't ship bindings for platform `{}({})`. try the `bindgen` feature instead.",
            target,
            env::var("BUILD_TARGET").unwrap_or("n/a".into())
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
fn bindgen<'a, D, H, X, K, V>(out_dir: D, header_file: H, defines: X, add_cflags: Vec<String>)
where
    D: AsRef<Path>,
    H: AsRef<Path>,
    X: IntoIterator<Item = &'a (K, Option<V>)>,
    K: AsRef<str> + 'a,
    V: AsRef<str> + 'a,
{
    let out_dir = out_dir.as_ref();
    let header_file = header_file.as_ref();

    let target = env::var("TARGET").unwrap();
    let host = env::var("HOST").unwrap();

    // When cross-compiling with the `macro` feature, sys also gets built for the host.
    // If LIBCLANG_PATH points at the cross toolchain (e.g. Android NDK), that host build
    // generates mismatched bindings, so reuse the bundled binding for the host instead.
    // `update-bindings` still regenerates.
    if target == host && env::var("CARGO_FEATURE_UPDATE_BINDINGS").is_err() {
        let bundled = Path::new("src")
            .join("bindings")
            .join(format!("{}.rs", target));
        if bundled.exists() {
            println!(
                "cargo:warning=using bundled bindings for host target `{}` instead of running bindgen (enable the `update-bindings` feature to regenerate)",
                target
            );
            fs::copy(&bundled, out_dir.join("bindings.rs"))
                .expect("Unable to copy bundled bindings");
            return;
        }
    }

    let mut cflags = add_cflags;

    //format!("-I{}", out_dir.parent().display()),

    for (name, value) in defines {
        cflags.push(if let Some(value) = value {
            format!("-D{}={}", name.as_ref(), value.as_ref())
        } else {
            format!("-D{}", name.as_ref())
        });
    }

    let mut builder = bindgen_rs::Builder::default()
        .use_core()
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
        .blocklist_function("JS_DumpMemoryUsage");

    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "wasi" {
        builder = builder.clang_arg("-fvisibility=default");
    }

    let bindings = builder.generate().expect("Unable to generate bindings");

    let bindings_file = out_dir.join("bindings.rs");

    bindings
        .write_to_file(&bindings_file)
        .expect("Couldn't write bindings");

    // Special case to support bundled bindings
    if env::var("CARGO_FEATURE_UPDATE_BINDINGS").is_ok() {
        let dest_dir = Path::new("src").join("bindings");
        fs::create_dir_all(&dest_dir).unwrap();

        let dest_file = format!("{}.rs", env::var("TARGET").unwrap());
        fs::copy(&bindings_file, dest_dir.join(dest_file)).unwrap();
    }
}
