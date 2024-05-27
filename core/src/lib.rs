//! # High-level bindings to QuickJS
//!
//! The `rquickjs` crate provides safe high-level bindings to the [QuickJS](https://bellard.org/quickjs/) JavaScript engine.
//! This crate is heavily inspired by the [rlua](https://crates.io/crates/rlua) crate.

#![allow(clippy::needless_lifetimes)]
#![cfg_attr(feature = "doc-cfg", feature(doc_cfg))]

pub(crate) use std::{result::Result as StdResult, string::String as StdString};

pub mod markers;
mod persistent;
mod result;
mod safe_ref;
mod util;
mod value;
pub(crate) use safe_ref::*;
pub mod runtime;
pub use runtime::Runtime;
pub mod context;
pub use context::{Context, Ctx};
pub mod class;
pub use class::Class;
pub use persistent::{Outlive, Persistent};
pub use result::{CatchResultExt, CaughtError, CaughtResult, Error, Result, ThrowResultExt};
pub use value::{
    array, atom, convert, function, iterator, module, object, promise, Array, Atom, BigInt,
    Coerced, Exception, Filter, FromAtom, FromIteratorJs, FromJs, Function, IntoAtom, IntoJs,
    Iterable, Iterator, IteratorJs, Module, Null, Object, Promise, String, Symbol, Type, Undefined,
    Value,
};

#[cfg(feature = "allocator")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "allocator")))]
pub mod allocator;
#[cfg(feature = "loader")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "loader")))]
pub mod loader;

#[cfg(feature = "futures")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
pub use context::AsyncContext;
#[cfg(feature = "multi-ctx")]
pub use context::MultiWith;
#[cfg(feature = "futures")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
pub use runtime::AsyncRuntime;
#[cfg(feature = "array-buffer")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "array-buffer")))]
pub use value::{ArrayBuffer, TypedArray};

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
/// Make sure the string does not contain any internal null characters or it will panic.
#[macro_export]
macro_rules! cstr {
    ($str:tt) => {{
        const fn no_null(s: &[u8]) {
            let mut i = 0;
            while i < s.len() {
                if s[i] == 0 {
                    panic!("C-str string contained null character")
                }
                i += 1;
            }
        }
        no_null($str.as_bytes());
        unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(concat!($str, "\0").as_bytes()) }
    }};
}

pub mod prelude {
    //! A group of often used types.
    #[cfg(feature = "multi-ctx")]
    pub use crate::context::MultiWith;
    pub use crate::{
        context::Ctx,
        convert::{Coerced, FromAtom, FromIteratorJs, FromJs, IntoAtom, IntoJs, IteratorJs, List},
        function::{
            Exhaustive, Flat, Func, FuncArg, IntoArg, IntoArgs, MutFn, OnceFn, Opt, Rest, This,
        },
        result::{CatchResultExt, ThrowResultExt},
    };
    #[cfg(feature = "futures")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    pub use crate::{
        function::Async,
        promise::{Promise, Promised},
    };
}

#[cfg(test)]
pub(crate) fn test_with<F, R>(func: F) -> R
where
    F: FnOnce(Ctx) -> R,
{
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();
    ctx.with(func)
}
