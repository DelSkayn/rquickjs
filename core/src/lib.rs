//! # High-level bindings to quickjs
//!
//! The `rquickjs` crate provides safe high-level bindings to the [quickjs](https://bellard.org/quickjs/) javascript engine.
//! This crate is heavily inspired by the [rlua](https://crates.io/crates/rlua) crate.

#![allow(clippy::needless_lifetimes)]
#![cfg_attr(feature = "doc-cfg", feature(doc_cfg))]

//#[doc(hidden)]
pub mod qjs {
    //! Native low-level bindings
    pub use rquickjs_sys::*;
}

#[cfg(feature = "phf")]
#[doc(hidden)]
pub mod phf {
    pub use phf::*;
}

/// Short macro to define a cstring literal.
///
/// Make sure the string does not contain any internal null characters or it panic.
#[macro_export]
macro_rules! cstr {
    ($str:tt) => {{
        const fn no_null(s: &[u8]) {
            let mut i = 0;
            while i < s.len() {
                if s[i] == 0 {
                    panic!("cstr string contained null character")
                }
                i += 1;
            }
        }
        no_null($str.as_bytes());
        unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(concat!($str, "\0").as_bytes()) }
    }};
}

pub mod markers;
mod result;
pub use result::{CatchResultExt, CaughtError, CaughtResult, Error, Result, ThrowResultExt};
mod safe_ref;
pub(crate) use safe_ref::*;
pub mod runtime;
#[cfg(feature = "futures")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
pub use runtime::AsyncRuntime;
pub use runtime::Runtime;
pub mod context;
#[cfg(feature = "futures")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
pub use context::AsyncContext;
pub use context::{Context, Ctx};
mod persistent;
mod value;
pub use persistent::{Outlive, Persistent};
pub use value::{
    array, atom, convert, function, module, object, Array, Atom, BigInt, Exception, FromAtom,
    FromJs, Function, IntoAtom, IntoJs, Module, Null, Object, String, Symbol, Type, Undefined,
    Value,
};

pub mod class;

#[cfg(feature = "array-buffer")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "array-buffer")))]
pub use value::{ArrayBuffer, TypedArray};

pub(crate) use std::{result::Result as StdResult, string::String as StdString};

#[cfg(feature = "futures")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
pub mod promise;

#[cfg(feature = "allocator")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "allocator")))]
pub mod allocator;

#[cfg(feature = "loader")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "loader")))]
pub mod loader;

pub mod prelude {
    //! A group of often used types.
    #[cfg(feature = "futures")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    pub use crate::promise::{Promise, Promised};
    pub use crate::{
        context::MultiWith,
        convert::{Coerced, FromAtom, FromJs, IntoAtom, IntoJs, IteratorJs},
        result::{CatchResultExt, ThrowResultExt},
    };
}

/*#[cfg(feature = "loader")]
pub use loader::{
    BuiltinLoader, BuiltinResolver, Bundle, Compile, FileResolver, HasByteCode, Loader,
    ModuleLoader, Resolver, ScriptLoader,
};
*/

#[cfg(test)]
pub(crate) fn test_with<F, R>(func: F) -> R
where
    F: FnOnce(Ctx) -> R,
{
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();
    ctx.with(func)
}
