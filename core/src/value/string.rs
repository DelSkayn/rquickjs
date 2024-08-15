use crate::{qjs, Ctx, Error, Result, StdString, Value};
use std::{mem, slice, str};

/// Rust representation of a JavaScript string.
#[derive(Debug, Clone, PartialEq, Hash)]
#[repr(transparent)]
pub struct String<'js>(pub(crate) Value<'js>);

impl<'js> AsRef<str> for String<'js>{
    fn as_ref(&self) -> &str {
        self.as_str().unwrap()
    }
}

impl<'js> String<'js> {
    /// Convert the JavaScript string to a Rust string.
    pub fn to_string(&self) -> Result<StdString> {
        let (str, ptr) = self.get_str_ref()?;
        let string = str.to_string();
        unsafe { qjs::JS_FreeCString(self.0.ctx.as_ptr(), ptr) };
        Ok(string)
    }

    /// Convert the JavaScript string to a string slice.
    pub fn as_str(&self) -> Result<&str> {
        let (str,_) = self.get_str_ref()?;
        Ok(str)
    }

    fn get_str_ref(&self) -> Result<(&str,*const i8)> {
        let mut len = mem::MaybeUninit::uninit();
        let ptr = unsafe {
            qjs::JS_ToCStringLen(self.0.ctx.as_ptr(), len.as_mut_ptr(), self.0.as_js_value())
        };
        if ptr.is_null() {
            // Might not ever happen but I am not 100% sure
            // so just incase check it.
            return Err(Error::Unknown);
        }
        let len = unsafe { len.assume_init() };
        let bytes: &[u8] = unsafe { slice::from_raw_parts(ptr as _, len as _) };
        let result = if cfg!(debug_assertions) {
            str::from_utf8(bytes)?
        } else {
            unsafe { str::from_utf8_unchecked(bytes) }
        };
        Ok((result,ptr))
    }
    
    /// Create a new JavaScript string from an Rust string.
    pub fn from_str(ctx: Ctx<'js>, s: &str) -> Result<Self> {
        let len = s.as_bytes().len();
        let ptr = s.as_ptr();
        Ok(unsafe {
            let js_val = qjs::JS_NewStringLen(ctx.as_ptr(), ptr as _, len as _);
            let js_val = ctx.handle_exception(js_val)?;
            String::from_js_value(ctx, js_val)
        })
    }
}

#[cfg(test)]
mod test {
    use crate::{prelude::*, *};
    #[test]
    fn from_javascript() {
        test_with(|ctx| {
            let s: String = ctx.eval(" 'foo bar baz' ").unwrap();
            assert_eq!(s.as_str().unwrap(), "foo bar baz");
            assert_eq!(s.to_string().unwrap(), "foo bar baz");
        });
    }

    #[test]
    fn to_javascript() {
        test_with(|ctx| {
            let string = String::from_str(ctx.clone(), "foo").unwrap();
            let func: Function = ctx.eval("x =>  x + 'bar'").unwrap();
            let text: StdString = (string,).apply(&func).unwrap();
            assert_eq!(text, "foobar".to_string());

            let s: String = ctx.eval(" '▧ ▧' ").unwrap();

            let str = s.as_str().unwrap();
    
            // alloc some memory to mangle the string.
            for _ in 0..100 {
                let s: String = ctx.eval(" 'foo' ").unwrap();
                s.to_string().unwrap();
            }
    
            assert_eq!(str, "▧ ▧");

        });
    }
}
