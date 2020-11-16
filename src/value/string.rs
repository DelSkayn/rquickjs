use crate::{
    context::Ctx,
    value::{self, rf::JsStringRef},
    Error, Result,
};
use rquickjs_sys as qjs;
use std::{ffi::CStr, mem, string::String as StdString};

/// Rust representation of a javascript string.
#[derive(Debug, Clone, PartialEq)]
pub struct String<'js>(pub(crate) JsStringRef<'js>);

impl<'js> String<'js> {
    /// Convert the javascript string to a rust string.
    pub fn to_string(&self) -> Result<StdString> {
        pub struct DropStr<'js>(Ctx<'js>, *const i8);

        impl<'js> Drop for DropStr<'js> {
            fn drop(&mut self) {
                unsafe {
                    qjs::JS_FreeCString(self.0.ctx, self.1);
                }
            }
        }

        unsafe {
            let c_str = qjs::JS_ToCString(self.0.ctx.ctx, self.0.as_js_value());
            // Ensure the c_string is dropped no matter what happens
            let drop = DropStr(self.0.ctx, c_str);
            if c_str.is_null() {
                // Might not ever happen but I am not 100% sure
                // so just incase check it.
                return Err(Error::Unknown);
            }
            let res = CStr::from_ptr(c_str).to_str()?.to_string();
            mem::drop(drop);
            Ok(res)
        }
    }

    /// Create a new js string from an rust string.
    pub fn from_str(ctx: Ctx<'js>, s: &str) -> Result<Self> {
        unsafe {
            let len = s.len();
            let bytes = s.as_ptr() as *const i8;
            let js_val = qjs::JS_NewStringLen(ctx.ctx, bytes, len as _);
            let js_val = value::handle_exception(ctx, js_val)?;
            Ok(String(JsStringRef::from_js_value(ctx, js_val)))
        }
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    #[test]
    fn from_javascript() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let val = ctx.eval::<Value, _>(" 'foo bar baz' ");
            if let Ok(Value::String(x)) = val {
                assert_eq!(x.to_string().unwrap(), "foo bar baz".to_string())
            } else {
                panic!("val not a string");
            };
        });
    }

    #[test]
    fn to_javascript() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let string = String::from_str(ctx, "foo").unwrap();
            let func = ctx.eval::<Function, _>("(x) =>  x + 'bar'").unwrap();
            let text: StdString = func.call(string).unwrap();
            assert_eq!(text, "foobar".to_string());
        });
    }
}
