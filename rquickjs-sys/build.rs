use cc;
use std::{env, fs, path::Path, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_PARALLEL");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_EXPORTS");

    let out_dir_env = env::var_os("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir_env);

    let opt_level = env::var_os("OPT_LEVEL")
        .unwrap()
        .to_str()
        .unwrap()
        .parse()
        .unwrap();

    if env::var("CARGO_FEATURE_EXPORTS").is_ok() || env::var("CARGO_FEATURE_PARALLEL").is_ok() {
        Command::new("patch")
            .arg("quickjs/quickjs.c")
            .arg("patches/quickjs.c.diff")
            .arg("-o")
            .arg(out_dir.join("quickjs.c"))
            .output()
            .unwrap();
    } else {
        fs::copy("quickjs/quickjs.c", out_dir.join("quickjs.c")).unwrap();
    }

    let source_files = [
        "libregexp.c",
        "libunicode.c",
        "cutils.c",
        "quickjs-libc.c",
        "libbf.c",
    ];
    let header_files = [
        "libbf.h",
        "libregexp-opcode.h",
        "libregexp.h",
        "libunicode-table.h",
        "libunicode.h",
        "list.h",
        "quickjs-atom.h",
        "quickjs-libc.h",
        "quickjs-opcode.h",
        "quickjs.h",
        "cutils.h",
    ];
    for e in source_files.iter().chain(header_files.iter()) {
        fs::copy(Path::new("quickjs").join(e), out_dir.join(e)).unwrap();
    }

    let mut builder = cc::Build::new();
    builder
        .extra_warnings(false)
        .flag("-Wno-array-bounds")
        .flag("-Wno-format-truncation")
        .flag("-g")
        .define("_GNU_SOURCE", None)
        .define("CONFIG_VERSION", "\"2020-01-19\"")
        .define("CONFIG_BIGNUM", None)
        .opt_level(opt_level)
        .file(out_dir.join("quickjs.c"));

    for e in source_files.iter() {
        builder.file(out_dir.join(e));
    }

    if let Ok(_) = env::var("CARGO_FEATURE_EXPORTS") {
        builder.define("CONFIG_MODULE_EXPORTS", None);
    }
    if let Ok(_) = env::var("CARGO_FEATURE_PARALLEL") {
        builder.define("CONFIG_PARALLEL", None);
    }
    builder.compile("libquickjs.a");
}
