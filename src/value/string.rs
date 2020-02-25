use crate::{
    context::Ctx,
    value::{self, rf::JsStringRef},
    Error, Result,
};
use rquickjs_sys as qjs;
use std::ffi::CStr;

/// Rust representation of a javascript string.
#[derive(Debug, Clone, PartialEq)]
pub struct String<'js>(JsStringRef<'js>);

impl<'js> String<'js> {
    // Unsafe because pointers must be valid and the
    // liftime of this object must within the lifetime of the context
    // Further more the JSValue must also be of type string as indicated by JS_TAG_STRING
    // All save functions rely on this constrained to be save
    pub(crate) unsafe fn from_js_value(ctx: Ctx<'js>, v: qjs::JSValue) -> Self {
        String(JsStringRef::from_js_value(ctx, v))
    }

    // Save because using the JSValue is unsafe
    pub(crate) fn as_js_value(&self) -> qjs::JSValue {
        self.0.as_js_value()
    }

    /// Convert the javascript string to a rust string.
    pub fn to_str(&self) -> Result<&str> {
        unsafe {
            let c_str = qjs::JS_ToCString(self.0.ctx.ctx, self.as_js_value());
            if c_str.is_null() {
                // Might not ever happen but I am not 100% sure
                // so just incase check it.
                return Err(Error::Unknown);
            }
            Ok(CStr::from_ptr(c_str).to_str()?)
        }
    }

    pub fn from_str(ctx: Ctx<'js>, s: &str) -> Result<Self> {
        unsafe {
            let len = s.len();
            let bytes = s.as_ptr() as *const i8;
            let js_val = qjs::JS_NewStringLen(ctx.ctx, bytes, len as u64);
            let js_val = value::handle_exception(ctx, js_val)?;
            assert_eq!(js_val.tag, qjs::JS_TAG_STRING as i64);
            Ok(String::from_js_value(ctx, js_val))
        }
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    #[test]
    fn js_value_string_from_javascript() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let val = ctx.eval::<Value, _>(" 'foo bar baz' ");
            if let Ok(Value::String(x)) = val {
                assert_eq!(x.to_str(), Ok("foo bar baz"))
            } else {
                panic!();
            };
        });
    }
}
