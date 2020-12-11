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

mod markers;
pub use markers::SendWhenParallel;
mod result;
use result::{get_exception, handle_exception, handle_panic};
pub use result::{Error, Result};
mod safe_ref;
pub(crate) use safe_ref::{SafeRef, SafeWeakRef};
mod runtime;
pub use runtime::Runtime;
mod context;
pub use context::{Context, ContextBuilder, Ctx, MultiWith};
mod value;
pub use value::*;
mod persistent;
pub use persistent::{Outlive, Persistent};

#[cfg(feature = "registery")]
mod registery_key;
#[cfg(feature = "registery")]
pub use registery_key::RegisteryKey;

#[cfg(feature = "classes")]
mod class;
#[cfg(feature = "classes")]
pub use class::{Class, ClassDef, ClassId, Constructor, RefsMarker, WithProto};

#[cfg(feature = "properties")]
mod property;
#[cfg(feature = "properties")]
pub use property::{AsProperty, NoConfigurable, NoEnumerable, NoWritable};

pub(crate) use std::{result::Result as StdResult, string::String as StdString};

#[doc(hidden)]
pub use rquickjs_sys as qjs;

pub(crate) mod async_shim;

#[cfg(any(feature = "tokio", feature = "async-std"))]
pub use crate::async_shim::JoinHandle;

#[cfg(feature = "futures")]
mod promise;

#[cfg(feature = "macro")]
pub use rquickjs_macro::{bind, FromJs, IntoJs};

#[cfg(feature = "futures")]
pub use promise::{Promise, PromiseJs};

#[cfg(feature = "allocator")]
mod allocator;

#[cfg(feature = "allocator")]
pub use allocator::{Allocator, RawMemPtr};

#[cfg(feature = "loader")]
mod loader;

#[cfg(feature = "loader")]
pub use loader::{
    BuiltinLoader, BuiltinResolver, FileResolver, Loader, ModuleLoader, Resolver, ScriptLoader,
};

#[cfg(feature = "dyn-load")]
pub use loader::NativeLoader;

#[cfg(test)]
pub(crate) fn test_with<'js, F, R>(func: F) -> R
where
    F: FnOnce(Ctx) -> R,
{
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();
    ctx.with(func)
}
