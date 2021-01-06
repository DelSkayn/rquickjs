//! # High-level bindings to quickjs
//!
//! The `rquickjs` crate provides safe high-level bindings to the [quickjs](https://bellard.org/quickjs/) javascript engine.
//! This crate is heavily inspired by the [rlua](https://crates.io/crates/rlua) crate.
//!
//! # The `Runtime` and `Context` objects
//!
//! The main entry point of this library is the [`Runtime`] struct.
//! It represents the interperter state and is used to create [`Context`]
//! objects. As the quickjs library does not support threading the runtime is locked behind a
//! mutex. Multiple threads cannot run as script or create objects from the same runtime at the
//! same time.
//! The [`Context`] object represents a global environment and a stack. Contexts of the same runtime
//! can share javascript objects like in browser between frames of the same origin.
//!
//! # Converting Values
//!
//! This library has multiple traits for converting to and from javascript.
//! The [`IntoJs`] trait are used for taking rust values and turning them into javascript values.
//! The [`FromJs`] is for converting javascript value to rust.
//! Note that this trait does some automatic coercion.
//! For values which represent the name of variables or indecies the
//! trait [`IntoAtom`] is available to convert values to the represention
//! quickjs requires.

#![allow(clippy::needless_lifetimes)]
#![cfg_attr(feature = "doc-cfg", feature(doc_cfg))]

#[cfg(feature = "async-std")]
extern crate async_std_rs as async_std;

#[cfg(feature = "tokio")]
extern crate tokio_rs as tokio;

#[cfg(feature = "phf")]
#[doc(hidden)]
pub use phf;

mod markers;
pub use markers::SendWhenParallel;
mod result;
use result::{get_exception, handle_exception, handle_panic};
pub use result::{Error, Result};
mod safe_ref;
pub(crate) use safe_ref::*;
mod runtime;
pub use runtime::Runtime;
#[cfg(feature = "futures")]
pub use runtime::{Executor, ExecutorSpawner, Idle};
mod context;
pub use context::{intrinsic, Context, ContextBuilder, Ctx, Intrinsic, MultiWith};
mod value;
pub use value::*;
mod persistent;
pub use persistent::{Outlive, Persistent};

mod class_id;
#[cfg(not(feature = "classes"))]
pub(crate) use class_id::ClassId;
#[cfg(feature = "classes")]
pub use class_id::ClassId;

#[cfg(feature = "registery")]
mod registery_key;
#[cfg(feature = "registery")]
pub use registery_key::RegisteryKey;

#[cfg(feature = "classes")]
mod class;
#[cfg(feature = "classes")]
pub use class::{Class, ClassDef, Constructor, HasRefs, RefsMarker, WithProto};

#[cfg(feature = "properties")]
mod property;
#[cfg(feature = "properties")]
pub use property::{Accessor, AsProperty, Property};

pub(crate) use std::{result::Result as StdResult, string::String as StdString};

#[doc(hidden)]
pub use rquickjs_sys as qjs;

#[cfg(feature = "futures")]
mod promise;

#[cfg(feature = "futures")]
pub use promise::{Promise, PromiseJs};

#[cfg(feature = "allocator")]
mod allocator;

#[cfg(feature = "allocator")]
pub use allocator::{Allocator, RawMemPtr, RustAllocator};

#[cfg(feature = "loader")]
mod loader;

#[cfg(feature = "loader")]
pub use loader::{
    BuiltinLoader, BuiltinResolver, Bundle, Compile, FileResolver, HasByteCode, Loader,
    ModuleLoader, Resolver, ScriptLoader,
};

#[cfg(feature = "dyn-load")]
pub use loader::NativeLoader;

/// A marker type to support the __tokio__ async runtime
#[cfg(feature = "tokio")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "tokio")))]
pub struct Tokio;

/// A marker type to support the __async-std__ runtime
#[cfg(feature = "async-std")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "async-std")))]
pub struct AsyncStd;

#[cfg(test)]
pub(crate) fn test_with<'js, F, R>(func: F) -> R
where
    F: FnOnce(Ctx) -> R,
{
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();
    ctx.with(func)
}
