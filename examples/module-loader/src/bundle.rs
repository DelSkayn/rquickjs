#[cfg(not(feature = "macro"))]
mod hand_written;

#[cfg(feature = "macro")]
mod using_macro;

#[cfg(not(feature = "macro"))]
pub use hand_written::NativeModule;

#[cfg(feature = "macro")]
pub use using_macro::NativeModule;

pub const SCRIPT_MODULE: &str = r#"
export const n = 123;
export const s = "abc";
export const f = (a, b) => (a + b) * 0.5;
"#;
