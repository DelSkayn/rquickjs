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
/// Make sure the string does not contain internal null characters or it will end early.
#[macro_export]
macro_rules! cstr {
    ($str:tt) => {
        std::ffi::CStr::from_bytes_until_nul(concat!($str, "\0").as_bytes()).unwrap()
    };
}

pub mod markers;
mod result;
pub use result::{CatchResultExt, CaughtError, CaughtResult, Error, Result, ThrowResultExt};
mod safe_ref;
pub(crate) use safe_ref::*;
pub mod runtime;
#[cfg(feature = "futures")]
pub use runtime::AsyncRuntime;
pub use runtime::Runtime;
pub mod context;
#[cfg(feature = "futures")]
pub use context::AsyncContext;
pub use context::{Context, Ctx};
mod persistent;
mod value;
pub use persistent::{Outlive, Persistent};
pub use value::{
    convert, function, module, object, Array, Atom, BigInt, Exception, FromAtom, FromJs, Function,
    IntoAtom, IntoJs, Module, Null, Object, String, Symbol, Type, Undefined, Value,
};

#[cfg(feature = "array-buffer")]
pub use value::{ArrayBuffer, TypedArray};
mod class_id;
#[cfg(not(feature = "classes"))]
pub(crate) use class_id::ClassId;
#[cfg(feature = "classes")]
pub use class_id::ClassId;

#[cfg(feature = "classes")]
pub mod class;
#[cfg(feature = "classes")]
pub use class::Class;

pub(crate) use std::{result::Result as StdResult, string::String as StdString};

#[cfg(feature = "futures")]
pub mod promise;

#[cfg(feature = "allocator")]
pub mod allocator;

#[cfg(feature = "loader")]
pub mod loader;

pub mod prelude {
    //! A group of often used types.
    pub use crate::{
        context::MultiWith,
        convert::{Coerced, FromAtom, FromJs, IntoAtom, IntoJs, IteratorJs},
        function::{AsArguments, Func, MutFn, OnceFn, Rest, This},
        result::{CatchResultExt, ThrowResultExt},
    };
    #[cfg(feature = "futures")]
    pub use crate::{
        function::Async,
        promise::{Promise, Promised},
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
