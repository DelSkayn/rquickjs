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
//! The [`IntoJs`], [`IntoJsArgs`] traits are used for taking rust values
//! and turning them into javascript values.
//! [`IntoJsArgs`] is specificly used for place where a specific number of values
//! need to be converted to javascript like for example the arguments of functions.
//! [`FromJs`] is for converting javascript value to rust.
//! Note that this trait does some automatic coercion.
//! For values which represent the name of variables or indecies the
//! trait [`IntoAtom`] is available to convert values to the represention
//! quickjs requires.
//!
//!
//! [`Runtime`]: struct.Runtime.html
//! [`Context`]: struct.Context.html
//! [`IntoJs`]: trait.IntoJs.html
//! [`IntoJsMulti`]: trait.IntoJsMulti.html
//! [`FromJs`]: trait.FromJs.html
//! [`IntoAtom`]: trait.IntoAtom.html

#![allow(clippy::needless_lifetimes)]

use quick_error::quick_error;
use std::{ffi::NulError, io, str};

mod context;
mod registery_key;
pub use registery_key::RegisteryKey;
mod runtime;
mod safe_ref;
pub use context::{Context, ContextBuilder, Ctx, MultiWith};
pub use runtime::Runtime;
mod markers;
mod value;
use std::result::Result as StdResult;
use std::string::String as StdString;
pub use value::*;

#[cfg(feature = "futures")]
mod promise;

#[cfg(feature = "futures")]
pub use promise::{Promise, PromiseJs};

quick_error! {
    /// Error type of the library.
    #[derive(Debug)]
    pub enum Error{
        /// Could not allocate memory
        /// This is generally only triggered when out of memory.
        Allocation{
            display("Allocation failed while creating object")
        }
        /// Found a string with a internal null byte while converting
        /// to C string.
        InvalidString(e: NulError){
            display("string contained internal null bytes: {}",e)
            from()
            cause(e)
        }
        /// String from rquickjs was not UTF-8
        Utf8(e: str::Utf8Error){
            display("Conversion from string failed: {}",e)
            from()
            cause(e)
        }
        /// An error from quickjs which i do not know the specifics about yet.
        /// Should eventually be removed as development progresses.
        Unknown{
            display("quickjs library created a unknown error")
        }
        /// An exception raised by quickjs itself.
        Exception{message: StdString, file: StdString, line: u32, stack: StdString}{
            display("exception generated by quickjs: [{}]:{} {}\n{}",file, line, message,stack)
        }
        /// Error converting from javascript to a rust type.
        FromJs{from: &'static str, to: &'static str, message: Option<StdString>} {
            display("error converting from js from type '{}', to '{}': {}",from,to,message.as_ref().unwrap_or(&StdString::new()))
        }
        /// Error converting to javascript from a rust type.
        IntoJs{from: &'static str, to: &'static str, message: Option<StdString>} {
            display("error converting from type '{}', to '{}': {}",from,to,message.as_ref().unwrap_or(&StdString::new()))
        }
        /// An io error
        IO(e: io::Error){
            display("IO Error: {}",e)
            from()
            cause(e)
        }
    }
}

impl Error {
    /// Returns wheter the error is a quickjs generated exception.
    pub fn is_exception(&self) -> bool {
        matches!(*self, Error::Exception{..})
    }
}

impl<'js> FromJs<'js> for Error {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let obj = Object::from_js(ctx, value)?;
        if obj.is_error() {
            Ok(Error::Exception {
                message: obj.get("message")?,
                file: obj.get("fileName").unwrap_or_else(|_| "unknown".into()),
                line: obj.get::<_, f64>("lineNumber")? as u32,
                stack: obj.get("stack")?,
            })
        } else {
            Err(Error::FromJs {
                from: "object",
                to: "error",
                message: None,
            })
        }
    }
}

/// Result type used throught the library.
pub type Result<T> = StdResult<T, Error>;

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn base_runtime() {
        let _rt = Runtime::new().unwrap();
    }
}
