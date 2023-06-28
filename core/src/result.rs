use std::{
    error::Error as StdError,
    ffi::{CString, FromBytesWithNulError, NulError},
    fmt::{self, Display, Formatter, Result as FmtResult},
    io::Error as IoError,
    panic,
    panic::UnwindSafe,
    str::{FromStr, Utf8Error},
    string::FromUtf8Error,
};

#[cfg(feature = "futures")]
use crate::context::AsyncContext;
use crate::{
    class::BorrowError, function::CellFnError, qjs, Context, Ctx, Exception, Object, StdResult,
    StdString, Type, Value,
};

/// Result type used throught the library.
pub type Result<T> = StdResult<T, Error>;

/// Result type containing an the javascript exception if there was one.
pub type CaughtResult<'js, T> = StdResult<T, CaughtError<'js>>;

/// Error type of the library.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Could not allocate memory
    /// This is generally only triggered when out of memory.
    Allocation,
    /// A module defined two exported values with the same name.
    DuplicateExports,
    /// Found a string with a internal null byte while converting
    /// to C string.
    InvalidString(NulError),
    /// Found a string with a internal null byte while converting
    /// to C string.
    InvalidCStr(FromBytesWithNulError),
    /// String from rquickjs was not UTF-8
    Utf8(Utf8Error),
    /// An io error
    Io(IoError),
    /// An error happened while trying to borrow a rust class object.
    Borrow(BorrowError),
    /// An error happened while trying to borrow a rust function.
    CellFn(CellFnError),
    /// An exception raised by quickjs itself.
    /// The actual javascript value can be retrieved by calling [`Ctx::catch`].
    ///
    /// When returned from a callback the javascript will continue to unwind with the current
    /// error.
    Exception,
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
    /// Error matching of function arguments
    MissingArgs {
        expected: usize,
        given: usize,
    },
    TooManyArgs {
        expected: usize,
        given: usize,
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
    /// Error when restoring a Persistent in a runtime other than the original runtime.
    UnrelatedRuntime,
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
        matches!(self, Error::Exception)
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
        matches!(self, Self::FromJs { .. })
    }

    /// Returns whether the error is a from JS to JS type conversion error
    pub fn is_from_js_to_js(&self) -> bool {
        matches!(self, Self::FromJs { to, .. } if Type::from_str(to).is_ok())
    }

    /// Returns whether the error is an into JS conversion error
    pub fn is_into_js(&self) -> bool {
        matches!(self, Self::IntoJs { .. })
    }

    /// Return whether the error is an function args mismatch error
    pub fn is_num_args(&self) -> bool {
        matches!(self, Self::TooManyArgs { .. } | Self::MissingArgs { .. })
    }

    /// Optimized conversion to CString
    pub(crate) fn to_cstring(&self) -> CString {
        // stringify error with NUL at end
        let mut message = format!("{self}\0").into_bytes();

        message.pop(); // pop last NUL because CString add this later

        // TODO: Replace by `CString::from_vec_with_nul_unchecked` later when it will be stabilized
        unsafe { CString::from_vec_unchecked(message) }
    }

    /// Throw an exception
    pub(crate) fn throw(&self, ctx: Ctx) -> qjs::JSValue {
        use Error::*;
        match self {
            Exception => qjs::JS_EXCEPTION,
            Allocation => unsafe { qjs::JS_ThrowOutOfMemory(ctx.as_ptr()) },
            InvalidString(_)
            | Utf8(_)
            | FromJs { .. }
            | IntoJs { .. }
            | TooManyArgs { .. }
            | MissingArgs { .. } => {
                let message = self.to_cstring();
                unsafe { qjs::JS_ThrowTypeError(ctx.as_ptr(), message.as_ptr()) }
            }
            #[cfg(feature = "loader")]
            Resolving { .. } | Loading { .. } => {
                let message = self.to_cstring();
                unsafe { qjs::JS_ThrowReferenceError(ctx.as_ptr(), message.as_ptr()) }
            }
            Unknown => {
                let message = self.to_cstring();
                unsafe { qjs::JS_ThrowInternalError(ctx.as_ptr(), message.as_ptr()) }
            }
            error => {
                unsafe {
                    let value = qjs::JS_NewError(ctx.as_ptr());
                    if qjs::JS_VALUE_GET_NORM_TAG(value) == qjs::JS_TAG_EXCEPTION {
                        //allocation error happened, can't raise error properly. just immediately
                        //return
                        return value;
                    }
                    let obj = Object::from_js_value(ctx, value);
                    match obj.set("message", error.to_string()) {
                        Ok(_) => {}
                        Err(Error::Exception) => return qjs::JS_EXCEPTION,
                        Err(e) => {
                            panic!("generated error while throwing error: {}", e);
                        }
                    }
                    std::mem::drop(obj);
                    todo!()
                    //let js_val = (obj).into_js_value();
                    //return qjs::JS_Throw(ctx.as_ptr(), js_val);
                }
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
            DuplicateExports => {
                "Tried to export two values with the same name from one module".fmt(f)?
            }
            InvalidString(error) => {
                "String contained internal null bytes: ".fmt(f)?;
                error.fmt(f)?;
            }
            InvalidCStr(error) => {
                "CStr didn't end in a null byte: ".fmt(f)?;
                error.fmt(f)?;
            }
            Utf8(error) => {
                "Conversion from string failed: ".fmt(f)?;
                error.fmt(f)?;
            }
            Unknown => "quickjs library created a unknown error".fmt(f)?,
            Exception => "Exception generated by quickjs".fmt(f)?,
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
            MissingArgs { expected, given } => {
                "Error calling function with ".fmt(f)?;
                given.fmt(f)?;
                " argument(s) while ".fmt(f)?;
                expected.fmt(f)?;
                " where expected".fmt(f)?;
            }
            TooManyArgs { expected, given } => {
                "Error calling function with ".fmt(f)?;
                given.fmt(f)?;
                " argument(s), function is exhaustive and cannot be called with more then "
                    .fmt(f)?;
                expected.fmt(f)?;
                " arguments".fmt(f)?;
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
            Io(error) => {
                "IO Error: ".fmt(f)?;
                error.fmt(f)?;
            }
            Borrow(x) => x.fmt(f)?,
            CellFn(x) => x.fmt(f)?,
            UnrelatedRuntime => "Restoring Persistent in an unrelated runtime".fmt(f)?,
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
    FromBytesWithNulError => InvalidCStr,
    Utf8Error => Utf8,
    IoError => Io,
    BorrowError => Borrow,
    CellFnError => CellFn,
}

impl From<FromUtf8Error> for Error {
    fn from(error: FromUtf8Error) -> Self {
        Error::Utf8(error.utf8_error())
    }
}

/// An error type containing possible thrown exception values.
#[derive(Debug)]
pub enum CaughtError<'js> {
    /// Error wasn't an exception
    Error(Error),
    /// Error was an exception and an instance of Error
    Exception(Exception<'js>),
    /// Error was an exception but not an instance of Error.
    Value(Value<'js>),
}

impl<'js> Display for CaughtError<'js> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match *self {
            CaughtError::Error(ref e) => e.fmt(f),
            CaughtError::Exception(ref e) => e.fmt(f),
            CaughtError::Value(ref e) => {
                writeln!(f, "Exception generated by quickjs: {e:?}")
            }
        }
    }
}

impl<'js> StdError for CaughtError<'js> {}

impl<'js> CaughtError<'js> {
    /// Create a `CaughtError` from an [`Error`], retrieving the error value from `Ctx` if there
    /// was one.
    pub fn from_error(ctx: Ctx<'js>, error: Error) -> Self {
        if let Error::Exception = error {
            let value = ctx.catch();
            if let Some(ex) = value
                .as_object()
                .and_then(|x| Exception::from_object(x.clone()))
            {
                CaughtError::Exception(ex)
            } else {
                CaughtError::Value(value)
            }
        } else {
            CaughtError::Error(error)
        }
    }

    /// Turn a `Result` with [`Error`] into a result with [`CaughtError`] retrieving the error
    /// value from the context if there was one.
    pub fn catch<T>(ctx: Ctx<'js>, error: Result<T>) -> CaughtResult<'js, T> {
        error.map_err(|error| Self::from_error(ctx, error))
    }

    /// Put the possible caught value back as the current error and turn the [`CaughtError`] into [`Error`]
    pub fn throw(self, ctx: Ctx<'js>) -> Error {
        match self {
            CaughtError::Error(e) => e,
            CaughtError::Exception(ex) => ctx.throw(ex.into_value()),
            CaughtError::Value(ex) => ctx.throw(ex),
        }
    }

    /// Returns whether self is of variant `CaughtError::Exception`.
    pub fn is_exception(&self) -> bool {
        matches!(self, CaughtError::Exception(_))
    }

    /// Returns whether self is of variant `CaughtError::Exception` or `CaughtError::Value`.
    pub fn is_js_error(&self) -> bool {
        matches!(self, CaughtError::Exception(_) | CaughtError::Value(_))
    }
}

/// Extension trait to easily turn results with [`Error`] into results with [`CaughtError`]
/// # Usage
/// ```
/// # use rquickjs::{Error, Context, Runtime, CaughtError};
/// # let rt = Runtime::new().unwrap();
/// # let ctx = Context::full(&rt).unwrap();
/// # ctx.with(|ctx|{
/// use rquickjs::CatchResultExt;
///
/// if let Err(CaughtError::Value(err)) = ctx.eval::<(),_>("throw 3").catch(ctx){
///     assert_eq!(err.as_int(),Some(3));
/// # }else{
/// #    panic!()
/// }
/// # });
/// ```
pub trait CatchResultExt<'js, T> {
    fn catch(self, ctx: Ctx<'js>) -> CaughtResult<'js, T>;
}

impl<'js, T> CatchResultExt<'js, T> for Result<T> {
    fn catch(self, ctx: Ctx<'js>) -> CaughtResult<'js, T> {
        CaughtError::catch(ctx, self)
    }
}

/// Extension trait to easily turn results with [`CaughtError`] into results with [`Error`]
///
/// Calling throw on a `CaughtError` will set the current error to the one contained in
/// `CaughtError` if such a value exists and then turn `CaughtError` into `Error`.
pub trait ThrowResultExt<'js, T> {
    fn throw(self, ctx: Ctx<'js>) -> Result<T>;
}

impl<'js, T> ThrowResultExt<'js, T> for CaughtResult<'js, T> {
    fn throw(self, ctx: Ctx<'js>) -> Result<T> {
        self.map_err(|e| e.throw(ctx))
    }
}

/// A error raised from running a pending job
/// Contains the context from which the error was raised.
///
/// Use `Ctx::catch` to retrieve the error.
#[derive(Clone)]
pub struct JobException(pub Context);

impl fmt::Debug for JobException {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_tuple("JobException")
            .field(&"TODO: Context")
            .finish()
    }
}

impl Display for JobException {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Job raised an exception")?;
        // TODO print the error?
        Ok(())
    }
}

/// A error raised from running a pending job
/// Contains the context from which the error was raised.
///
/// Use `Ctx::catch` to retrieve the error.
#[cfg(feature = "futures")]
#[derive(Clone)]
pub struct AsyncJobException(pub AsyncContext);

#[cfg(feature = "futures")]
impl fmt::Debug for AsyncJobException {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_tuple("AsyncJobException")
            .field(&"TODO: Context")
            .finish()
    }
}

#[cfg(feature = "futures")]
impl Display for AsyncJobException {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Async job raised an exception")?;
        // TODO print the error?
        Ok(())
    }
}

impl<'js> Ctx<'js> {
    pub(crate) fn handle_panic<F>(self, f: F) -> qjs::JSValue
    where
        F: FnOnce() -> qjs::JSValue + UnwindSafe,
    {
        unsafe {
            match panic::catch_unwind(f) {
                Ok(x) => x,
                Err(e) => {
                    self.get_opaque().panic = Some(e);
                    qjs::JS_Throw(self.as_ptr(), qjs::JS_MKVAL(qjs::JS_TAG_EXCEPTION, 0))
                }
            }
        }
    }

    /// Handle possible exceptions in JSValue's and turn them into errors
    /// Will return the JSValue if it is not an exception
    ///
    /// # Safety
    /// Assumes to have ownership of the JSValue
    pub(crate) unsafe fn handle_exception(self, js_val: qjs::JSValue) -> Result<qjs::JSValue> {
        if qjs::JS_VALUE_GET_NORM_TAG(js_val) != qjs::JS_TAG_EXCEPTION {
            Ok(js_val)
        } else {
            if let Some(x) = self.get_opaque().panic.take() {
                panic::resume_unwind(x)
            }
            Err(Error::Exception)
        }
    }

    /// Returns Error::Exception if there is no existing panic,
    /// otherwise continues panicking.
    pub(crate) fn raise_exception(self) -> Error {
        // Safety
        unsafe {
            if let Some(x) = self.get_opaque().panic.take() {
                panic::resume_unwind(x)
            }
            Error::Exception
        }
    }
}
