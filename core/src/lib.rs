//! # High-level bindings to quickjs
//!
//! The `rquickjs` crate provides safe high-level bindings to the [quickjs](https://bellard.org/quickjs/) javascript engine.
//! This crate is heavily inspired by the [rlua](https://crates.io/crates/rlua) crate.

#![allow(clippy::needless_lifetimes)]
#![cfg_attr(feature = "doc-cfg", feature(doc_cfg))]

#[cfg(feature = "async-std")]
extern crate async_std_rs as async_std;

#[cfg(feature = "tokio")]
extern crate tokio_rs as tokio;

#[cfg(feature = "smol")]
extern crate smol_rs as smol;

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

mod markers;
pub use markers::ParallelSend;
mod result;
use result::{get_exception, handle_exception, handle_panic};
pub use result::{Error, Result};
mod safe_ref;
pub(crate) use safe_ref::*;
mod runtime;
#[cfg(feature = "async-std")]
pub use runtime::AsyncStd;
#[cfg(all(feature = "smol", feature = "parallel"))]
pub use runtime::Smol;
#[cfg(feature = "tokio")]
pub use runtime::Tokio;
#[cfg(feature = "futures")]
pub use runtime::{Executor, ExecutorSpawner, Idle};
pub use runtime::{MemoryUsage, Runtime};
mod context;
pub use context::{intrinsic, Context, ContextBuilder, Ctx, EvalOptions, Intrinsic, MultiWith};
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

#[cfg(feature = "futures")]
mod promise;

#[cfg(feature = "futures")]
pub use promise::{Promise, Promised};

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

#[cfg(test)]
pub(crate) fn test_with<F, R>(func: F) -> R
where
    F: FnOnce(Ctx) -> R,
{
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();
    ctx.with(func)
}
