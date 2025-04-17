//! # High-level bindings to QuickJS
//!
//! The `rquickjs` crate provides safe high-level bindings to the [QuickJS](https://bellard.org/quickjs/) JavaScript engine.
//! This crate is heavily inspired by the [rlua](https://crates.io/crates/rlua) crate.

#![allow(unknown_lints)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::uninlined_format_args)]
#![allow(mismatched_lifetime_syntaxes)]
#![cfg_attr(feature = "doc-cfg", feature(doc_cfg))]
#![allow(clippy::doc_lazy_continuation)]
#![cfg_attr(not(test), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std as alloc;

pub(crate) use alloc::string::String as StdString;
pub(crate) use core::result::Result as StdResult;

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
    array, atom, convert, function, module, object, promise, Array, Atom, BigInt, CString, Coerced,
    Exception, Filter, FromAtom, FromIteratorJs, FromJs, Function, IntoAtom, IntoJs, IteratorJs,
    Module, Null, Object, Promise, String, Symbol, Type, Undefined, Value, WriteOptions,
    WriteOptionsEndianness,
};

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

mod deprecated_features {
    #[cfg(feature = "properties")]
    #[allow(unused_imports)]
    use properties as _;
    #[cfg(feature = "properties")]
    #[deprecated(
        note = "The rquickjs crate feature `properties` is deprecated, the functionality it provided is now enabled by default.
To remove this warning remove the use of the feature when specifying the dependency."
    )]
    mod properties {}

    #[cfg(feature = "array-buffer")]
    #[allow(unused_imports)]
    use array_buffer as _;
    #[cfg(feature = "array-buffer")]
    #[deprecated(
        note = "The rquickjs crate feature `array-buffer` is deprecated, the functionality it provided is now enabled by default.
To remove this warning remove the use of the feature when specifying the dependency."
    )]
    mod array_buffer {}

    #[cfg(feature = "classes")]
    #[allow(unused_imports)]
    use classes as _;
    #[cfg(feature = "classes")]
    #[deprecated(
        note = "The rquickjs crate feature `classes` is deprecated, the functionality it provided is now enabled by default.
To remove this warning remove the use of the feature when specifying the dependency."
    )]
    mod classes {}

    #[cfg(feature = "allocator")]
    #[allow(unused_imports)]
    use allocator as _;
    #[cfg(feature = "allocator")]
    #[deprecated(
        note = "The rquickjs crate feature `allocator` is deprecated, the functionality it provided is now enabled by default.
To remove this warning remove the use of the feature when specifying the dependency."
    )]
    mod allocator {}
}
