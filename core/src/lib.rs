//! # High-level bindings to QuickJS
//!
//! The `rquickjs` crate provides safe high-level bindings to the [QuickJS](https://bellard.org/quickjs/) JavaScript engine.
//! This crate is heavily inspired by the [rlua](https://crates.io/crates/rlua) crate.

#![allow(clippy::needless_lifetimes)]
#![cfg_attr(feature = "doc-cfg", feature(doc_cfg))]

pub(crate) use std::{result::Result as StdResult, string::String as StdString};

mod js_lifetime;
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
pub use js_lifetime::JsLifetime;
pub use persistent::Persistent;
pub use result::{CatchResultExt, CaughtError, CaughtResult, Error, Result, ThrowResultExt};
pub use value::{
    array, atom, convert, function, module, object, promise, Array, Atom, BigInt, Coerced,
    Exception, Filter, FromAtom, FromIteratorJs, FromJs, Function, IntoAtom, IntoJs, IteratorJs,
    Module, Null, Object, Promise, String, Symbol, Type, Undefined, Value,
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
        JsLifetime,
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
