use std::fmt;

use crate::{atom::PredefinedAtom, convert::Coerced, qjs, Ctx, Error, Object, Result, Value};

/// A javascript instance of Error
///
/// Will turn into a error when converted to javascript but won't autmatically be thrown.
#[repr(transparent)]
pub struct Exception<'js>(pub(crate) Object<'js>);

impl fmt::Debug for Exception<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Exception")
            .field("object", &self.0)
            .field("message", &self.message())
            .field("file", &self.file())
            .field("line", &self.line())
            .field("stack", &self.stack())
            .finish()
    }
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

    /// Creates a new exception with a give message.
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

    /// Creates a new exception with a give message, file name and line number.
    pub fn from_message_location(
        ctx: Ctx<'js>,
        message: &str,
        file: &str,
        line: i32,
    ) -> Result<Self> {
        let obj = unsafe {
            let value = ctx.handle_exception(qjs::JS_NewError(ctx.as_ptr()))?;
            Value::from_js_value(ctx, value)
                .into_object()
                .expect("`JS_NewError` did not return an object")
        };
        obj.set(PredefinedAtom::Message, message)?;
        obj.set(PredefinedAtom::FileName, file)?;
        obj.set(PredefinedAtom::LineNumber, line)?;
        Ok(Exception(obj))
    }

    /// Returns the message of the error.
    ///
    /// Same as retrieving `error.message` in javascript.
    pub fn message(&self) -> Option<String> {
        self.get::<_, Option<Coerced<String>>>("message")
            .ok()
            .and_then(|x| x)
            .map(|x| x.0)
    }

    /// Returns the file name from with the error originated..
    ///
    /// Same as retrieving `error.fileName` in javascript.
    pub fn file(&self) -> Option<String> {
        self.get::<_, Option<Coerced<String>>>(PredefinedAtom::FileName)
            .ok()
            .and_then(|x| x)
            .map(|x| x.0)
    }

    /// Returns the file line from with the error originated..
    ///
    /// Same as retrieving `error.lineNumber` in javascript.
    pub fn line(&self) -> Option<i32> {
        self.get::<_, Option<Coerced<i32>>>(PredefinedAtom::LineNumber)
            .ok()
            .and_then(|x| x)
            .map(|x| x.0)
    }

    /// Returns the error stack.
    ///
    /// Same as retrieving `error.stack` in javascript.
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
    pub fn throw_message(ctx: Ctx<'js>, message: &str) -> Error {
        let (Ok(e) | Err(e)) = Self::from_message(ctx, message).map(|x| x.throw());
        e
    }
    /// Throws a new generic error with a file name and line number.
    pub fn throw_message_location(ctx: Ctx<'js>, message: &str, file: &str, line: i32) -> Error {
        let (Ok(e) | Err(e)) =
            Self::from_message_location(ctx, message, file, line).map(|x| x.throw());
        e
    }

    /// Throws a new syntax error.
    pub fn throw_syntax(ctx: Ctx<'js>, message: &str) -> Error {
        // generate C string inline.
        // quickjs implementation doesn't allow error strings longer then 256 anyway so truncating
        // here is fine.
        let mut buffer = std::mem::MaybeUninit::<[u8; 256]>::uninit();
        let str_len = message.as_bytes().len().min(255);
        unsafe {
            std::ptr::copy_nonoverlapping(message.as_ptr(), buffer.as_mut_ptr().cast(), str_len);
            buffer.as_mut_ptr().cast::<u8>().add(str_len).write(b'\0');
            let res = qjs::JS_ThrowSyntaxError(ctx.as_ptr(), buffer.as_ptr().cast());
            debug_assert_eq!(qjs::JS_VALUE_GET_NORM_TAG(res), qjs::JS_TAG_EXCEPTION);
        }
        Error::Exception
    }

    /// Throws a new type error.
    pub fn throw_type(ctx: Ctx<'js>, message: &str) -> Error {
        // generate C string inline.
        // quickjs implementation doesn't allow error strings longer then 256 anyway so truncating
        // here is fine.
        let mut buffer = std::mem::MaybeUninit::<[u8; 256]>::uninit();
        let str_len = message.as_bytes().len().min(255);
        unsafe {
            std::ptr::copy_nonoverlapping(
                message.as_ptr(),
                buffer.as_mut_ptr().cast::<u8>(),
                str_len,
            );
            buffer.as_mut_ptr().cast::<u8>().add(str_len).write(b'\0');
            let res = qjs::JS_ThrowTypeError(ctx.as_ptr(), buffer.as_ptr().cast());
            debug_assert_eq!(qjs::JS_VALUE_GET_NORM_TAG(res), qjs::JS_TAG_EXCEPTION);
        }
        Error::Exception
    }

    /// Throws a new reference error.
    pub fn throw_reference(ctx: Ctx<'js>, message: &str) -> Error {
        // generate C string inline.
        // quickjs implementation doesn't allow error strings longer then 256 anyway so truncating
        // here is fine.
        let mut buffer = std::mem::MaybeUninit::<[u8; 256]>::uninit();
        let str_len = message.as_bytes().len().min(255);
        unsafe {
            std::ptr::copy_nonoverlapping(
                message.as_ptr(),
                buffer.as_mut_ptr().cast::<u8>(),
                str_len,
            );
            buffer.as_mut_ptr().cast::<u8>().add(str_len).write(b'\0');
            let res = qjs::JS_ThrowReferenceError(ctx.as_ptr(), buffer.as_ptr().cast());
            debug_assert_eq!(qjs::JS_VALUE_GET_NORM_TAG(res), qjs::JS_TAG_EXCEPTION);
        }
        Error::Exception
    }

    /// Throws a new range error.
    pub fn throw_range(ctx: Ctx<'js>, message: &str) -> Error {
        // generate C string inline.
        // quickjs implementation doesn't allow error strings longer then 256 anyway so truncating
        // here is fine.
        let mut buffer = std::mem::MaybeUninit::<[u8; 256]>::uninit();
        let str_len = message.as_bytes().len().min(255);
        unsafe {
            std::ptr::copy_nonoverlapping(
                message.as_ptr(),
                buffer.as_mut_ptr().cast::<u8>(),
                str_len,
            );
            buffer.as_mut_ptr().cast::<u8>().add(str_len).write(b'\0');
            let res = qjs::JS_ThrowRangeError(ctx.as_ptr(), buffer.as_ptr().cast());
            debug_assert_eq!(qjs::JS_VALUE_GET_NORM_TAG(res), qjs::JS_TAG_EXCEPTION);
        }
        Error::Exception
    }

    /// Throws a new internal error.
    pub fn throw_internal(ctx: Ctx<'js>, message: &str) -> Error {
        // generate C string inline.
        // quickjs implementation doesn't allow error strings longer then 256 anyway so truncating
        // here is fine.
        let mut buffer = std::mem::MaybeUninit::<[u8; 256]>::uninit();
        let str_len = message.as_bytes().len().min(255);
        unsafe {
            std::ptr::copy_nonoverlapping(
                message.as_ptr(),
                buffer.as_mut_ptr().cast::<u8>(),
                str_len,
            );
            buffer.as_mut_ptr().cast::<u8>().add(str_len).write(b'\0');
            let res = qjs::JS_ThrowInternalError(ctx.as_ptr(), buffer.as_ptr().cast());
            debug_assert_eq!(qjs::JS_VALUE_GET_NORM_TAG(res), qjs::JS_TAG_EXCEPTION);
        }
        Error::Exception
    }

    /// Sets the exception as the current error an returns `Error::Exception`
    pub fn throw(self) -> Error {
        self.0.ctx.throw(self.0.into_value())
    }
}

impl fmt::Display for Exception<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "Exception generated by quickjs: ".fmt(f)?;
        if let Some(file) = self.file() {
            '['.fmt(f)?;
            file.fmt(f)?;
            ']'.fmt(f)?;
        }
        if let Some(line) = self.line() {
            ':'.fmt(f)?;
            line.fmt(f)?;
        }
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
