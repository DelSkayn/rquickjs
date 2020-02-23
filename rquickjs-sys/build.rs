use cc;

fn main() {
    cc::Build::new()
        .extra_warnings(false)
        .flag("-Wno-array-bounds")
        .flag("-Wno-format-truncation")
        .define("_GNU_SOURCE", None)
        .define("CONFIG_VERSION", "\"2020-01-19\"")
        .define("CONFIG_BIGNUM", None)
        .opt_level(2)
        .file("quickjs/quickjs.c")
        .file("quickjs/libregexp.c")
        .file("quickjs/libunicode.c")
        .file("quickjs/cutils.c")
        .file("quickjs/quickjs-libc.c")
        .file("quickjs/libbf.c")
        .compile("libquickjs.a")
}
