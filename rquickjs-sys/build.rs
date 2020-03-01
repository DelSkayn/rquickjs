use cc;
use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_PARALLEL");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_EXPORTS");

    let mut builder = cc::Build::new();
    builder
        .extra_warnings(false)
        .flag("-Wno-array-bounds")
        .flag("-Wno-format-truncation")
        .flag("-g")
        .define("_GNU_SOURCE", None)
        .define("CONFIG_VERSION", "\"2020-01-19\"")
        .define("CONFIG_BIGNUM", None)
        //.opt_level(2)
        .file("quickjs/quickjs.c")
        .file("quickjs/libregexp.c")
        .file("quickjs/libunicode.c")
        .file("quickjs/cutils.c")
        .file("quickjs/quickjs-libc.c")
        .file("quickjs/libbf.c");

    if let Ok(_) = env::var("CARGO_FEATURE_EXPORTS") {
        builder.define("CONFIG_MODULE_EXPORTS", None);
    }
    if let Ok(_) = env::var("CARGO_FEATURE_PARALLEL") {
        builder.define("CONFIG_PARALLEL", None);
    }
    builder.compile("libquickjs.a");
}
