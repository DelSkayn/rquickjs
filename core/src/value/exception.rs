use alloc::string::String;
use core::{error::Error as ErrorTrait, ffi::CStr, fmt};

use crate::{atom::PredefinedAtom, convert::Coerced, qjs, Ctx, Error, Object, Result, Value};

/// A JavaScript instance of Error
///
/// Will turn into a error when converted to JavaScript but won't automatically be thrown.
#[repr(transparent)]
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct Exception<'js>(pub(crate) Object<'js>);

impl<'js> ErrorTrait for Exception<'js> {}

impl fmt::Debug for Exception<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Exception")
            .field("message", &self.message())
            .field("stack", &self.stack())
            .finish()
    }
}

pub(crate) static ERROR_FORMAT_STR: &CStr =
    unsafe { CStr::from_bytes_with_nul_unchecked("%s\0".as_bytes()) };

fn truncate_str(mut max: usize, bytes: &[u8]) -> &[u8] {
    if bytes.len() <= max {
        return bytes;
    }
    // while the byte at len is a continue byte shorten the byte.
    while (bytes[max] & 0b1100_0000) == 0b1000_0000 {
        max -= 1;
    }
    &bytes[..max]
}

impl<'js> Exception<'js> {
    /// Turns the exception into the underlying object.
    pub fn into_object(self) -> Object<'js> {
        self.0
    }

    /// Returns a reference to the underlying object.
    pub fn as_object(&self) -> &Object<'js> {
        &self.0
    }

    /// Creates an exception from an object if it is an instance of error.
    pub fn from_object(obj: Object<'js>) -> Option<Self> {
        if obj.is_error() {
            Some(Self(obj))
        } else {
            None
        }
    }

    /// Creates a new exception with a given message.
    pub fn from_message(ctx: Ctx<'js>, message: &str) -> Result<Self> {
        let obj = unsafe {
            let value = ctx.handle_exception(qjs::JS_NewError(ctx.as_ptr()))?;
            Value::from_js_value(ctx, value)
                .into_object()
                .expect("`JS_NewError` did not return an object")
        };
        obj.set(PredefinedAtom::Message, message)?;
        Ok(Exception(obj))
    }

    /// Returns the message of the error.
    ///
    /// Same as retrieving `error.message` in JavaScript.
    pub fn message(&self) -> Option<String> {
        self.get::<_, Option<Coerced<String>>>(PredefinedAtom::Message)
            .ok()
            .and_then(|x| x)
            .map(|x| x.0)
    }

    /// Returns the error stack.
    ///
    /// Same as retrieving `error.stack` in JavaScript.
    pub fn stack(&self) -> Option<String> {
        self.get::<_, Option<Coerced<String>>>(PredefinedAtom::Stack)
            .ok()
            .and_then(|x| x)
            .map(|x| x.0)
    }

    /// Throws a new generic error.
    ///
    /// Equivalent to:
    /// ```rust
    /// # use rquickjs::{Runtime,Context,Exception};
    /// # let rt = Runtime::new().unwrap();
    /// # let ctx = Context::full(&rt).unwrap();
    /// # ctx.with(|ctx|{
    /// # let _ = {
    /// # let message = "";
    /// let (Ok(e) | Err(e)) = Exception::from_message(ctx, message).map(|x| x.throw());
    /// e
    /// # };
    /// # })
    /// ```
    pub fn throw_message(ctx: &Ctx<'js>, message: &str) -> Error {
        let (Ok(e) | Err(e)) = Self::from_message(ctx.clone(), message).map(|x| x.throw());
        e
    }

    /// Throws a new syntax error.
    pub fn throw_syntax(ctx: &Ctx<'js>, message: &str) -> Error {
        // generate C string inline.
        // QuickJS implementation doesn't allow error strings longer then 256 anyway so truncating
        // here is fine.
        let mut buffer = core::mem::MaybeUninit::<[u8; 256]>::uninit();
        let str = truncate_str(255, message.as_bytes());
        unsafe {
            core::ptr::copy_nonoverlapping(message.as_ptr(), buffer.as_mut_ptr().cast(), str.len());
            buffer.as_mut_ptr().cast::<u8>().add(str.len()).write(b'\0');
            let res = qjs::JS_ThrowSyntaxError(
                ctx.as_ptr(),
                ERROR_FORMAT_STR.as_ptr(),
                buffer.as_ptr().cast::<*mut u8>(),
            );
            debug_assert_eq!(qjs::JS_VALUE_GET_NORM_TAG(res), qjs::JS_TAG_EXCEPTION);
        }
        Error::Exception
    }

    /// Throws a new type error.
    pub fn throw_type(ctx: &Ctx<'js>, message: &str) -> Error {
        // generate C string inline.
        // QuickJS implementation doesn't allow error strings longer then 256 anyway so truncating
        // here is fine.
        let mut buffer = core::mem::MaybeUninit::<[u8; 256]>::uninit();
        let str = truncate_str(255, message.as_bytes());
        unsafe {
            core::ptr::copy_nonoverlapping(message.as_ptr(), buffer.as_mut_ptr().cast(), str.len());
            buffer.as_mut_ptr().cast::<u8>().add(str.len()).write(b'\0');
            let res = qjs::JS_ThrowTypeError(
                ctx.as_ptr(),
                ERROR_FORMAT_STR.as_ptr(),
                buffer.as_ptr().cast::<*mut u8>(),
            );
            debug_assert_eq!(qjs::JS_VALUE_GET_NORM_TAG(res), qjs::JS_TAG_EXCEPTION);
        }
        Error::Exception
    }

    /// Throws a new reference error.
    pub fn throw_reference(ctx: &Ctx<'js>, message: &str) -> Error {
        // generate C string inline.
        // QuickJS implementation doesn't allow error strings longer then 256 anyway so truncating
        // here is fine.
        let mut buffer = core::mem::MaybeUninit::<[u8; 256]>::uninit();
        let str = truncate_str(255, message.as_bytes());
        unsafe {
            core::ptr::copy_nonoverlapping(message.as_ptr(), buffer.as_mut_ptr().cast(), str.len());
            buffer.as_mut_ptr().cast::<u8>().add(str.len()).write(b'\0');
            let res = qjs::JS_ThrowReferenceError(
                ctx.as_ptr(),
                ERROR_FORMAT_STR.as_ptr(),
                buffer.as_ptr().cast::<*mut u8>(),
            );
            debug_assert_eq!(qjs::JS_VALUE_GET_NORM_TAG(res), qjs::JS_TAG_EXCEPTION);
        }
        Error::Exception
    }

    /// Throws a new range error.
    pub fn throw_range(ctx: &Ctx<'js>, message: &str) -> Error {
        // generate C string inline.
        // QuickJS implementation doesn't allow error strings longer then 256 anyway so truncating
        // here is fine.
        let mut buffer = core::mem::MaybeUninit::<[u8; 256]>::uninit();
        let str = truncate_str(255, message.as_bytes());
        unsafe {
            core::ptr::copy_nonoverlapping(message.as_ptr(), buffer.as_mut_ptr().cast(), str.len());
            buffer.as_mut_ptr().cast::<u8>().add(str.len()).write(b'\0');
            let res = qjs::JS_ThrowRangeError(
                ctx.as_ptr(),
                ERROR_FORMAT_STR.as_ptr(),
                buffer.as_ptr().cast::<*mut u8>(),
            );
            debug_assert_eq!(qjs::JS_VALUE_GET_NORM_TAG(res), qjs::JS_TAG_EXCEPTION);
        }
        Error::Exception
    }

    /// Throws a new internal error.
    pub fn throw_internal(ctx: &Ctx<'js>, message: &str) -> Error {
        // generate C string inline.
        // QuickJS implementation doesn't allow error strings longer then 256 anyway so truncating
        // here is fine.
        let mut buffer = core::mem::MaybeUninit::<[u8; 256]>::uninit();
        let str = truncate_str(255, message.as_bytes());
        unsafe {
            core::ptr::copy_nonoverlapping(message.as_ptr(), buffer.as_mut_ptr().cast(), str.len());
            buffer.as_mut_ptr().cast::<u8>().add(str.len()).write(b'\0');
            let res = qjs::JS_ThrowInternalError(
                ctx.as_ptr(),
                ERROR_FORMAT_STR.as_ptr(),
                buffer.as_ptr().cast::<*mut u8>(),
            );
            debug_assert_eq!(qjs::JS_VALUE_GET_NORM_TAG(res), qjs::JS_TAG_EXCEPTION);
        }
        Error::Exception
    }

    /// Sets the exception as the current error an returns `Error::Exception`
    pub fn throw(self) -> Error {
        let ctx = self.ctx().clone();
        ctx.throw(self.0.into_value())
    }
}

impl fmt::Display for Exception<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "Error:".fmt(f)?;
        if let Some(message) = self.message() {
            ' '.fmt(f)?;
            message.fmt(f)?;
        }
        if let Some(stack) = self.stack() {
            '\n'.fmt(f)?;
            stack.fmt(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn from_javascript() {
        test_with(|ctx| {
            let val: Exception = ctx.eval(r#"new Error("test")"#).unwrap();
            assert_eq!(val.message().unwrap(), "test");
        });
    }

    #[test]
    fn from_javascript_proxy() {
        test_with(|ctx| {
            let val: Exception = ctx.eval(r#"new Proxy(new Error("test"), { get: (target, property) => target[property] })"#).unwrap();
            assert_eq!(val.message().unwrap(), "test");
        });
    }
}
