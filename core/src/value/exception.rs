use std::{fmt, ops::Deref};

use crate::{convert::Coerced, qjs, Ctx, Error, FromJs, IntoJs, Object, Result, Value};

/// A javascript instance of Error
///
/// Will turn into a error when converted to javascript but won't autmatically be thrown.
#[repr(transparent)]
pub struct Exception<'js>(Object<'js>);

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

    pub fn into_value(self) -> Value<'js> {
        self.0.into_value()
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
        obj.set("message", message)?;
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
        obj.set("message", message)?;
        obj.set("fileName", file)?;
        obj.set("lineNumber", line)?;
        Ok(Exception(obj))
    }

    pub fn message(&self) -> Option<String> {
        self.get::<_, Option<Coerced<String>>>("message")
            .ok()
            .and_then(|x| x)
            .map(|x| x.0)
    }

    pub fn file(&self) -> Option<String> {
        self.get::<_, Option<Coerced<String>>>("fileName")
            .ok()
            .and_then(|x| x)
            .map(|x| x.0)
    }

    pub fn line(&self) -> Option<i32> {
        self.get::<_, Option<Coerced<i32>>>("lineNumber")
            .ok()
            .and_then(|x| x)
            .map(|x| x.0)
    }

    pub fn stack(&self) -> Option<String> {
        self.get::<_, Option<Coerced<String>>>("stack")
            .ok()
            .and_then(|x| x)
            .map(|x| x.0)
    }

    /// Throws a new syntax error.
    pub fn throw_syntax(ctx: Ctx<'js>, message: &str) -> Error {
        // generate C string inline.
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

impl<'js> Deref for Exception<'js> {
    type Target = Object<'js>;

    fn deref(&self) -> &Self::Target {
        self.as_object()
    }
}

impl<'js> FromJs<'js> for Exception<'js> {
    fn from_js(_ctx: crate::Ctx<'js>, value: Value<'js>) -> Result<Self> {
        if let Some(obj) = value.as_object() {
            if obj.is_error() {
                return Ok(Exception(obj.clone()));
            } else {
                return Err(Error::FromJs {
                    from: value.type_name(),
                    to: "Exception",
                    message: Some("object was not an instance of error".to_string()),
                });
            }
        }
        return Err(Error::FromJs {
            from: value.type_name(),
            to: "Exception",
            message: Some("value was not a type".to_string()),
        });
    }
}

impl<'js> IntoJs<'js> for Exception<'js> {
    fn into_js(self, _ctx: crate::Ctx<'js>) -> Result<Value<'js>> {
        Ok(self.0.into_value())
    }
}
