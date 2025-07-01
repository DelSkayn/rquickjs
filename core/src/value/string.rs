use crate::{qjs, Ctx, Error, Result, StdString, Value};
use core::{ffi::c_char, mem, ptr::NonNull, slice, str};

/// Rust representation of a JavaScript string.
#[derive(Debug, Clone, PartialEq, Hash)]
#[repr(transparent)]
pub struct String<'js>(pub(crate) Value<'js>);

impl<'js> String<'js> {
    /// Convert the JavaScript string to a Rust string.
    pub fn to_string(&self) -> Result<StdString> {
        let (ptr, len) = self.get_ptr_len()?;
        let bytes: &[u8] = unsafe { slice::from_raw_parts(ptr as _, len as _) };
        let result = str::from_utf8(bytes).map(|s| s.into());
        unsafe { qjs::JS_FreeCString(self.0.ctx.as_ptr(), ptr) };
        Ok(result?)
    }

    pub fn to_string_lossy(&self) -> Result<StdString> {
        let (ptr, len) = self.get_ptr_len()?;
        let bytes: &[u8] = unsafe { slice::from_raw_parts(ptr as _, len as _) };
        let string = Self::replace_invalid_utf8_and_utf16(bytes);
        unsafe { qjs::JS_FreeCString(self.0.ctx.as_ptr(), ptr) };
        Ok(string)
    }

    fn get_ptr_len(&self) -> Result<(*const i8, usize)> {
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
        Ok((ptr, len))
    }

    fn replace_invalid_utf8_and_utf16(bytes: &[u8]) -> StdString {
        let mut result = StdString::with_capacity(bytes.len());
        let mut i = 0;

        while i < bytes.len() {
            let current = bytes[i];
            match current {
                // ASCII (1-byte)
                0x00..=0x7F => {
                    result.push(current as char);
                    i += 1;
                }
                // 2-byte UTF-8 sequence
                0xC0..=0xDF => {
                    if i + 1 < bytes.len() {
                        let next = bytes[i + 1];
                        if (next & 0xC0) == 0x80 {
                            let code_point = ((current as u32 & 0x1F) << 6) | (next as u32 & 0x3F);
                            if let Some(c) = char::from_u32(code_point) {
                                result.push(c);
                            } else {
                                result.push('�');
                            }
                            i += 2;
                        } else {
                            result.push('�');
                            i += 1;
                        }
                    } else {
                        result.push('�');
                        i += 1;
                    }
                }
                // 3-byte UTF-8 sequence
                0xE0..=0xEF => {
                    if i + 2 < bytes.len() {
                        let next1 = bytes[i + 1];
                        let next2 = bytes[i + 2];
                        if (next1 & 0xC0) == 0x80 && (next2 & 0xC0) == 0x80 {
                            let code_point = ((current as u32 & 0x0F) << 12)
                                | ((next1 as u32 & 0x3F) << 6)
                                | (next2 as u32 & 0x3F);
                            if let Some(c) = char::from_u32(code_point) {
                                result.push(c);
                            } else {
                                result.push('�');
                            }
                            i += 3;
                        } else {
                            result.push('�');
                            i += 1;
                        }
                    } else {
                        result.push('�');
                        i += 1;
                    }
                }
                // 4-byte UTF-8 sequence
                0xF0..=0xF7 => {
                    if i + 3 < bytes.len() {
                        let next1 = bytes[i + 1];
                        let next2 = bytes[i + 2];
                        let next3 = bytes[i + 3];
                        if (next1 & 0xC0) == 0x80
                            && (next2 & 0xC0) == 0x80
                            && (next3 & 0xC0) == 0x80
                        {
                            let code_point = ((current as u32 & 0x07) << 18)
                                | ((next1 as u32 & 0x3F) << 12)
                                | ((next2 as u32 & 0x3F) << 6)
                                | (next3 as u32 & 0x3F);
                            if let Some(c) = char::from_u32(code_point) {
                                result.push(c);
                            } else {
                                result.push('�');
                            }
                            i += 4;
                        } else {
                            result.push('�');
                            i += 1;
                        }
                    } else {
                        result.push('�');
                        i += 1;
                    }
                }
                // Invalid starting byte
                _ => {
                    result.push('�');
                    i += 1;
                }
            }
        }

        result
    }

    /// Convert the Javascript string to a Javascript C string.
    pub fn to_cstring(self) -> Result<CString<'js>> {
        CString::from_string(self)
    }

    /// Create a new JavaScript string from an Rust string.
    pub fn from_str(ctx: Ctx<'js>, s: &str) -> Result<Self> {
        let len = s.len();
        let ptr = s.as_ptr();
        Ok(unsafe {
            let js_val = qjs::JS_NewStringLen(ctx.as_ptr(), ptr as _, len as _);
            let js_val = ctx.handle_exception(js_val)?;
            String::from_js_value(ctx, js_val)
        })
    }
}

/// Rust representation of a JavaScript C string.
#[derive(Debug)]
pub struct CString<'js> {
    ptr: NonNull<c_char>,
    len: usize,
    ctx: Ctx<'js>,
}

impl<'js> CString<'js> {
    /// Create a new JavaScript C string from a JavaScript string.
    pub fn from_string(string: String<'js>) -> Result<Self> {
        let mut len = mem::MaybeUninit::uninit();
        // SAFETY: The pointer points to a JSString content which is ref counted
        let ptr = unsafe {
            qjs::JS_ToCStringLen(string.0.ctx.as_ptr(), len.as_mut_ptr(), string.as_raw())
        };
        if ptr.is_null() {
            // Might not ever happen but I am not 100% sure
            // so just incase check it.
            return Err(Error::Unknown);
        }
        let len = unsafe { len.assume_init() };
        Ok(Self {
            ptr: unsafe { NonNull::new_unchecked(ptr as *mut _) },
            len,
            ctx: string.0.ctx.clone(),
        })
    }

    /// Converts a `CString` to a raw pointer.
    pub fn as_ptr(&self) -> *const c_char {
        self.ptr.as_ptr() as *const _
    }

    /// Returns the length of this `CString`, in bytes (not chars or graphemes).
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if this `CString` has a length of zero, and `false` otherwise.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Extracts a string slice containing the entire `CString`.
    pub fn as_str(&self) -> &str {
        // SAFETY: The pointer points to a JSString content which is ref counted
        let bytes = unsafe { slice::from_raw_parts(self.ptr.as_ptr() as *const u8, self.len) };
        // SAFETY: The bytes are garanteed to be valid utf8 by QuickJS
        unsafe { str::from_utf8_unchecked(bytes) }
    }
}

impl<'js> Drop for CString<'js> {
    fn drop(&mut self) {
        unsafe { qjs::JS_FreeCString(self.ctx.as_ptr(), self.ptr.as_ptr()) };
    }
}

#[cfg(test)]
mod test {
    use crate::{prelude::*, *};
    #[test]
    fn from_javascript() {
        test_with(|ctx| {
            let s: String = ctx.eval(" 'foo bar baz' ").unwrap();
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
        });
    }

    #[test]
    fn from_javascript_c() {
        test_with(|ctx| {
            let s: CString = ctx.eval(" 'foo bar baz' ").unwrap();
            assert_eq!(s.as_str(), "foo bar baz");
        });
    }

    #[test]
    fn to_javascript_c() {
        test_with(|ctx| {
            let string = String::from_str(ctx.clone(), "foo")
                .unwrap()
                .to_cstring()
                .unwrap();
            let func: Function = ctx.eval("x =>  x + 'bar'").unwrap();
            let text: StdString = (string,).apply(&func).unwrap();
            assert_eq!(text, "foobar".to_string());
        });
    }

    #[test]
    fn utf8_sliced_string() {
        test_with(|ctx| {
            let string = String::from_str(ctx.clone(), "🌍🌎🌏").unwrap();

            assert_eq!(string.to_string().unwrap(), "🌍🌎🌏".to_string());
            assert_eq!(string.to_string_lossy().unwrap(), "🌍🌎🌏".to_string());

            let func: Function = ctx.eval("x => x.slice(1)").unwrap();
            let text: String = (string,).apply(&func).unwrap();
            let text = text.to_string_lossy().unwrap();

            assert_eq!(text, "�🌎🌏".to_string());
        });
    }
}
