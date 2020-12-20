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

pub use rquickjs_core::*;

#[cfg(feature = "macro")]
pub use rquickjs_macro::{bind, FromJs, HasRefs, IntoJs};
