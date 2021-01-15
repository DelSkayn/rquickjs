use crate::{qjs, Ctx, FromJs, IntoJs, Object, StdResult, StdString, Value};

use std::{
    error::Error as StdError,
    ffi::{CString, NulError},
    fmt::{Display, Formatter, Result as FmtResult},
    io::Error as IoError,
    panic,
    panic::UnwindSafe,
    str::Utf8Error,
    string::FromUtf8Error,
};

/// Result type used throught the library.
pub type Result<T> = StdResult<T, Error>;

/// Error type of the library.
#[derive(Debug)]
pub enum Error {
    /// Could not allocate memory
    /// This is generally only triggered when out of memory.
    Allocation,
    /// Found a string with a internal null byte while converting
    /// to C string.
    InvalidString(NulError),
    /// String from rquickjs was not UTF-8
    Utf8(Utf8Error),
    /// An io error
    IO(IoError),
    /// An exception raised by quickjs itself.
    Exception {
        message: StdString,
        file: StdString,
        line: i32,
        stack: StdString,
    },
    /// Error converting from javascript to a rust type.
    FromJs {
        from: &'static str,
        to: &'static str,
        message: Option<StdString>,
    },
    /// Error converting to javascript from a rust type.
    IntoJs {
        from: &'static str,
        to: &'static str,
        message: Option<StdString>,
    },
    #[cfg(feature = "loader")]
    /// Error when resolving js module
    Resolving {
        base: StdString,
        name: StdString,
        message: Option<StdString>,
    },
    #[cfg(feature = "loader")]
    /// Error when loading js module
    Loading {
        name: StdString,
        message: Option<StdString>,
    },
    /// An error from quickjs from which the specifics are unknown.
    /// Should eventually be removed as development progresses.
    Unknown,
}

impl Error {
    #[cfg(feature = "loader")]
    /// Create resolving error
    pub fn new_resolving<B, N>(base: B, name: N) -> Self
    where
        StdString: From<B> + From<N>,
    {
        Error::Resolving {
            base: base.into(),
            name: name.into(),
            message: None,
        }
    }

    #[cfg(feature = "loader")]
    /// Create resolving error with message
    pub fn new_resolving_message<B, N, M>(base: B, name: N, msg: M) -> Self
    where
        StdString: From<B> + From<N> + From<M>,
    {
        Error::Resolving {
            base: base.into(),
            name: name.into(),
            message: Some(msg.into()),
        }
    }

    #[cfg(feature = "loader")]
    /// Returns whether the error is a resolving error
    pub fn is_resolving(&self) -> bool {
        matches!(self, Error::Resolving { .. })
    }

    #[cfg(feature = "loader")]
    /// Create loading error
    pub fn new_loading<N>(name: N) -> Self
    where
        StdString: From<N>,
    {
        Error::Loading {
            name: name.into(),
            message: None,
        }
    }

    #[cfg(feature = "loader")]
    /// Create loading error
    pub fn new_loading_message<N, M>(name: N, msg: M) -> Self
    where
        StdString: From<N> + From<M>,
    {
        Error::Loading {
            name: name.into(),
            message: Some(msg.into()),
        }
    }

    #[cfg(feature = "loader")]
    /// Returns whether the error is a loading error
    pub fn is_loading(&self) -> bool {
        matches!(self, Error::Loading { .. })
    }

    /// Returns whether the error is a quickjs generated exception.
    pub fn is_exception(&self) -> bool {
        matches!(self, Error::Exception{..})
    }

    /// Create from JS conversion error
    pub fn new_from_js(from: &'static str, to: &'static str) -> Self {
        Error::FromJs {
            from,
            to,
            message: None,
        }
    }

    /// Create from JS conversion error with message
    pub fn new_from_js_message<M>(from: &'static str, to: &'static str, msg: M) -> Self
    where
        StdString: From<M>,
    {
        Error::FromJs {
            from,
            to,
            message: Some(msg.into()),
        }
    }

    /// Create into JS conversion error
    pub fn new_into_js(from: &'static str, to: &'static str) -> Self {
        Error::IntoJs {
            from,
            to,
            message: None,
        }
    }

    /// Create into JS conversion error with message
    pub fn new_into_js_message<M>(from: &'static str, to: &'static str, msg: M) -> Self
    where
        StdString: From<M>,
    {
        Error::IntoJs {
            from,
            to,
            message: Some(msg.into()),
        }
    }

    /// Returns whether the error is a from JS conversion error
    pub fn is_from_js(&self) -> bool {
        matches!(self, Error::FromJs { .. })
    }

    /// Returns whether the error is an into JS conversion error
    pub fn is_into_js(&self) -> bool {
        matches!(self, Error::IntoJs { .. })
    }

    /// Optimized conversion to CString
    pub(crate) fn to_cstring(&self) -> CString {
        // stringify error with NUL at end
        let mut message = format!("{}\0", self).into_bytes();

        message.pop(); // pop last NUL because CString add this later

        // TODO: Replace by `CString::from_vec_with_nul_unchecked` later when it will be stabilized
        unsafe { CString::from_vec_unchecked(message) }
    }

    /// Throw an exception
    pub(crate) fn throw(&self, ctx: Ctx) -> qjs::JSValue {
        use Error::*;
        match self {
            Allocation => unsafe { qjs::JS_ThrowOutOfMemory(ctx.ctx) },
            InvalidString(_) | Utf8(_) | FromJs { .. } | IntoJs { .. } => {
                let message = self.to_cstring();
                unsafe { qjs::JS_ThrowTypeError(ctx.ctx, message.as_ptr()) }
            }
            #[cfg(feature = "loader")]
            Resolving { .. } | Loading { .. } => {
                let message = self.to_cstring();
                unsafe { qjs::JS_ThrowReferenceError(ctx.ctx, message.as_ptr()) }
            }
            Unknown => {
                let message = self.to_cstring();
                unsafe { qjs::JS_ThrowInternalError(ctx.ctx, message.as_ptr()) }
            }
            _ => {
                let value = self.into_js(ctx).unwrap();
                unsafe { qjs::JS_Throw(ctx.ctx, value.into_js_value()) }
            }
        }
    }
}

impl StdError for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        use Error::*;

        match self {
            Allocation => "Allocation failed while creating object".fmt(f)?,
            InvalidString(error) => {
                "String contained internal null bytes: ".fmt(f)?;
                error.fmt(f)?;
            }
            Utf8(error) => {
                "Conversion from string failed: ".fmt(f)?;
                error.fmt(f)?;
            }
            Unknown => "quickjs library created a unknown error".fmt(f)?,
            Exception {
                file,
                line,
                message,
                stack,
            } => {
                "Exception generated by quickjs: ".fmt(f)?;
                if !file.is_empty() {
                    '['.fmt(f)?;
                    file.fmt(f)?;
                    ']'.fmt(f)?;
                }
                if *line >= 0 {
                    ':'.fmt(f)?;
                    line.fmt(f)?;
                }
                if !message.is_empty() {
                    ' '.fmt(f)?;
                    message.fmt(f)?;
                }
                if !stack.is_empty() {
                    '\n'.fmt(f)?;
                    stack.fmt(f)?;
                }
            }
            FromJs { from, to, message } => {
                "Error converting from js '".fmt(f)?;
                from.fmt(f)?;
                "' into type '".fmt(f)?;
                to.fmt(f)?;
                "'".fmt(f)?;
                if let Some(message) = message {
                    if !message.is_empty() {
                        ": ".fmt(f)?;
                        message.fmt(f)?;
                    }
                }
            }
            IntoJs { from, to, message } => {
                "Error converting from '".fmt(f)?;
                from.fmt(f)?;
                "' into js '".fmt(f)?;
                to.fmt(f)?;
                "'".fmt(f)?;
                if let Some(message) = message {
                    if !message.is_empty() {
                        ": ".fmt(f)?;
                        message.fmt(f)?;
                    }
                }
            }
            #[cfg(feature = "loader")]
            Resolving {
                base,
                name,
                message,
            } => {
                "Error resolving module '".fmt(f)?;
                name.fmt(f)?;
                "' from '".fmt(f)?;
                base.fmt(f)?;
                "'".fmt(f)?;
                if let Some(message) = message {
                    if !message.is_empty() {
                        ": ".fmt(f)?;
                        message.fmt(f)?;
                    }
                }
            }
            #[cfg(feature = "loader")]
            Loading { name, message } => {
                "Error loading module '".fmt(f)?;
                name.fmt(f)?;
                "'".fmt(f)?;
                if let Some(message) = message {
                    if !message.is_empty() {
                        ": ".fmt(f)?;
                        message.fmt(f)?;
                    }
                }
            }
            IO(error) => {
                "IO Error: ".fmt(f)?;
                error.fmt(f)?;
            }
        }
        Ok(())
    }
}

macro_rules! from_impls {
    ($($type:ty => $variant:ident,)*) => {
        $(
            impl From<$type> for Error {
                fn from(error: $type) -> Self {
                    Error::$variant(error)
                }
            }
        )*
    };
}

from_impls! {
    NulError => InvalidString,
    Utf8Error => Utf8,
    IoError => IO,
}

impl From<FromUtf8Error> for Error {
    fn from(error: FromUtf8Error) -> Self {
        Error::Utf8(error.utf8_error())
    }
}

impl<'js> FromJs<'js> for Error {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let obj = Object::from_js(ctx, value)?;
        if obj.is_error() {
            Ok(Error::Exception {
                message: obj.get("message").unwrap_or_else(|_| "".into()),
                file: obj.get("fileName").unwrap_or_else(|_| "".into()),
                line: obj.get("lineNumber").unwrap_or(-1),
                stack: obj.get("stack").unwrap_or_else(|_| "".into()),
            })
        } else {
            Err(Error::new_from_js("object", "error"))
        }
    }
}

impl<'js> IntoJs<'js> for &Error {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        use Error::*;
        let value = unsafe {
            Object::from_js_value(ctx, handle_exception(ctx, qjs::JS_NewError(ctx.ctx))?)
        };
        match self {
            Exception {
                message,
                file,
                line,
                stack,
            } => {
                if !message.is_empty() {
                    value.set("message", message)?;
                }
                if !file.is_empty() {
                    value.set("fileName", file)?;
                }
                if *line >= 0 {
                    value.set("lineNumber", *line)?;
                }
                if !stack.is_empty() {
                    value.set("stack", stack)?;
                }
            }
            error => {
                value.set("message", error.to_string())?;
            }
        }
        Ok(value.0)
    }
}

pub(crate) fn handle_panic<F: FnOnce() -> qjs::JSValue + UnwindSafe>(
    ctx: *mut qjs::JSContext,
    f: F,
) -> qjs::JSValue {
    unsafe {
        match panic::catch_unwind(f) {
            Ok(x) => x,
            Err(e) => {
                Ctx::from_ptr(ctx).get_opaque().panic = Some(e);
                qjs::JS_Throw(ctx, qjs::JS_MKVAL(qjs::JS_TAG_EXCEPTION, 0))
            }
        }
    }
}

/// Handle possible exceptions in JSValue's and turn them into errors
/// Will return the JSValue if it is not an exception
///
/// # Safety
/// Assumes to have ownership of the JSValue
pub(crate) unsafe fn handle_exception<'js>(
    ctx: Ctx<'js>,
    js_val: qjs::JSValue,
) -> Result<qjs::JSValue> {
    if qjs::JS_VALUE_GET_NORM_TAG(js_val) != qjs::JS_TAG_EXCEPTION {
        Ok(js_val)
    } else {
        Err(get_exception(ctx))
    }
}

pub(crate) unsafe fn get_exception<'js>(ctx: Ctx<'js>) -> Error {
    let exception_val = qjs::JS_GetException(ctx.ctx);

    if let Some(x) = ctx.get_opaque().panic.take() {
        panic::resume_unwind(x);
    }

    let exception = Value::from_js_value(ctx, exception_val);
    Error::from_js(ctx, exception).unwrap()
}
