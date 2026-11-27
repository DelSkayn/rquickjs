use crate::{qjs, util::Sealed, Ctx, Error, Result, StdString, Value};
use core::{
    ffi::c_char,
    marker::PhantomData,
    mem::{self, ManuallyDrop},
    ptr::NonNull,
    slice, str,
};

/// Rust representation of a JavaScript string.
#[derive(Debug, Clone, PartialEq, Hash)]
#[repr(transparent)]
pub struct String<'js>(pub(crate) Value<'js>);

impl<'js> String<'js> {
    /// Convert the JavaScript string to a Rust string.
    pub fn to_string(&self) -> Result<StdString> {
        let string = self.clone().to_cstring()?.validate()?;
        Ok(string.as_str().into())
    }

    /// Convert the JavaScript string to a Rust string, replacing invalid UTF-8
    /// sequences with the Unicode replacement character (`U+FFFD`).
    pub fn to_string_lossy(&self) -> Result<StdString> {
        let string = self.clone().to_cstring()?;
        Ok(StdString::from_utf8_lossy(string.as_bytes()).into_owned())
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

/// Encoding marker for a JavaScript C string.
pub trait CStringEncoding: Sealed + 'static {}

/// A JavaScript C string validated as WTF-8.
#[derive(Debug, Clone, Copy)]
pub struct Wtf8;
impl Sealed for Wtf8 {}
impl CStringEncoding for Wtf8 {}

/// A JavaScript C string validated as UTF-8.
#[derive(Debug, Clone, Copy)]
pub struct Utf8;
impl Sealed for Utf8 {}
impl CStringEncoding for Utf8 {}

/// Rust representation of a JavaScript C string.
///
/// QuickJS strings may contain WTF-8. The default `CString<'js, Wtf8>` exposes
/// bytes only; call [`CString::validate`] to obtain a `CString<'js, Utf8>` with
/// access to UTF-8 string slices.
#[derive(Debug)]
pub struct CString<'js, E: CStringEncoding = Wtf8> {
    ptr: NonNull<c_char>,
    len: usize,
    ctx: Ctx<'js>,
    _encoding: PhantomData<E>,
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
            _encoding: PhantomData,
        })
    }

    /// Validate the bytes as UTF-8 without copying the string.
    ///
    /// Returns an error if the string contains invalid UTF-8.
    pub fn validate(self) -> Result<CString<'js, Utf8>> {
        str::from_utf8(self.as_bytes())?;
        let this = ManuallyDrop::new(self);
        Ok(CString {
            ptr: this.ptr,
            len: this.len,
            // SAFETY: `this` will not be dropped; ownership of its context is transferred.
            ctx: unsafe { core::ptr::read(&this.ctx) },
            _encoding: PhantomData,
        })
    }
}

impl<'js, E: CStringEncoding> CString<'js, E> {
    /// Create a JavaScript string value in the given context, preserving WTF-8.
    pub fn to_value(&self, ctx: &Ctx<'js>) -> Result<Value<'js>> {
        // SAFETY: The pointer is valid for `len` bytes, and QuickJS accepts WTF-8.
        unsafe {
            let value = qjs::JS_NewStringLen(ctx.as_ptr(), self.as_ptr(), self.len() as _);
            let value = ctx.handle_exception(value)?;
            Ok(Value::from_js_value(ctx.clone(), value))
        }
    }

    /// Extracts the bytes of this string, excluding the trailing NUL.
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: The allocation remains alive until this CString is dropped.
        unsafe { slice::from_raw_parts(self.ptr.as_ptr().cast(), self.len) }
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
}

impl<'js> CString<'js, Utf8> {
    /// Extracts a string slice containing the entire validated `CString`.
    pub fn as_str(&self) -> &str {
        // SAFETY: Only UTF-8 validation can construct this specialization.
        unsafe { str::from_utf8_unchecked(self.as_bytes()) }
    }
}

impl<'js, E: CStringEncoding> Drop for CString<'js, E> {
    fn drop(&mut self) {
        unsafe { qjs::JS_FreeCString(self.ctx.as_ptr(), self.ptr.as_ptr()) };
    }
}
impl<'js> AsRef<str> for CString<'js, Utf8> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
#[cfg(feature = "std")]
impl<'js> AsRef<std::path::Path> for CString<'js, Utf8> {
    fn as_ref(&self) -> &std::path::Path {
        std::path::Path::new(self.as_str())
    }
}
impl<'js> core::ops::Deref for CString<'js, Utf8> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}
#[cfg(feature = "std")]
impl<'js> AsRef<std::ffi::OsStr> for CString<'js, Utf8> {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.as_str().as_ref()
    }
}
impl<'js, E: CStringEncoding> AsRef<[u8]> for CString<'js, E> {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}
impl<'js, E: CStringEncoding, Other: CStringEncoding> PartialEq<CString<'js, Other>>
    for CString<'js, E>
{
    fn eq(&self, other: &CString<'js, Other>) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}
impl<'js, E: CStringEncoding> Eq for CString<'js, E> {}
impl<'js, E: CStringEncoding> core::hash::Hash for CString<'js, E> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        core::hash::Hash::hash(self.as_bytes(), state)
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
    fn string_conversions() {
        test_with(|ctx| {
            let string: String = ctx.eval(r#""hello\u0000\u{1f600}""#).unwrap();
            let cstring = string.clone().to_cstring().unwrap();
            assert_eq!(string.to_string().unwrap(), "hello\0😀");
            assert_eq!(string.to_string_lossy().unwrap(), "hello\0😀");
            let owned = CString::from_string(string).unwrap();
            assert_eq!(cstring, owned);
            assert_eq!(cstring.as_bytes(), "hello\0😀".as_bytes());

            let invalid: String = ctx.eval(r#""\ud800""#).unwrap();
            assert!(matches!(invalid.to_string(), Err(Error::Utf8(_))));
            assert_eq!(
                invalid.clone().to_cstring().unwrap().as_bytes(),
                &[0xed, 0xa0, 0x80]
            );
            assert!(invalid.to_string_lossy().is_ok());
        });
    }

    #[test]
    fn to_string_lossy() {
        test_with(|ctx| {
            for (source, expected) in [
                (r#""""#, ""),
                (r#""hello\u0000\u{1f600}""#, "hello\0😀"),
                (r#""a\ud800b\udfffc""#, "a���b���c"),
                (r#""\ud83d\ude00""#, "😀"),
            ] {
                let string: String = ctx.eval(source).unwrap();
                assert_eq!(string.to_string_lossy().unwrap(), expected);
            }
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
            let s: CString<Utf8> = ctx.eval(" 'foo bar baz' ").unwrap();
            assert_eq!(s.as_str(), "foo bar baz");
        });
    }

    #[test]
    fn cstring_validation() {
        test_with(|ctx| {
            let string: CString = ctx.eval(r#""hello\u0000\u{1f600}""#).unwrap();
            let expected = "hello\0😀";
            assert_eq!(string.as_bytes(), expected.as_bytes());
            assert_eq!(AsRef::<[u8]>::as_ref(&string), expected.as_bytes());
            assert_eq!(string.len(), expected.len());
            assert!(!string.is_empty());
            let ptr = string.as_ptr();
            let validated = string.validate().unwrap();
            assert_eq!(validated.as_ptr(), ptr);
            assert_eq!(validated.as_str(), expected);
            assert_eq!(validated.as_bytes(), expected.as_bytes());
            assert_eq!(AsRef::<str>::as_ref(&validated), expected);
            assert_eq!(&*validated, expected);

            let empty: CString = ctx.eval("''").unwrap();
            assert!(empty.is_empty());
            assert_eq!(empty.validate().unwrap().as_str(), "");
        });
    }

    #[test]
    fn cstring_lone_surrogates() {
        test_with(|ctx| {
            for (source, bytes) in [
                (r#""\ud800""#, &[0xed, 0xa0, 0x80][..]),
                (r#""\udfff""#, &[0xed, 0xbf, 0xbf][..]),
            ] {
                let string: CString = ctx.eval(source).unwrap();
                assert_eq!(string.as_bytes(), bytes);
                assert_eq!(AsRef::<[u8]>::as_ref(&string), bytes);
                assert!(matches!(string.validate(), Err(Error::Utf8(_))));
                assert!(matches!(
                    ctx.eval::<CString<Utf8>, _>(source),
                    Err(Error::Utf8(_))
                ));
            }
        });
    }

    #[test]
    fn cstring_wtf8_round_trip() {
        test_with(|ctx| {
            for source in [r#""\ud800""#, r#""\udfff""#, r#""a\u0000\ud800\u{1f600}z""#] {
                let string: CString = ctx.eval(source).unwrap();
                let value = (&string).into_js(&ctx).unwrap();
                let borrowed: CString = CString::from_js(&ctx, value).unwrap();
                assert_eq!(borrowed.as_bytes(), string.as_bytes());

                let value = string.into_js(&ctx).unwrap();
                let owned: CString = CString::from_js(&ctx, value).unwrap();
                assert_eq!(owned.as_bytes(), borrowed.as_bytes());
            }
        });
    }

    #[test]
    fn cstring_equality_and_hash() {
        use core::hash::{Hash, Hasher};

        test_with(|ctx| {
            let raw: CString = ctx.eval("'hello'").unwrap();
            let validated: CString<Utf8> = ctx.eval("'hello'").unwrap();
            assert_eq!(raw, validated);
            let mut raw_hash = std::collections::hash_map::DefaultHasher::new();
            let mut validated_hash = std::collections::hash_map::DefaultHasher::new();
            raw.hash(&mut raw_hash);
            validated.hash(&mut validated_hash);
            assert_eq!(raw_hash.finish(), validated_hash.finish());
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

            let validated: CString<Utf8> = ctx.eval("'foo'").unwrap();
            let text: StdString = (&validated,).apply(&func).unwrap();
            assert_eq!(text, "foobar");
            let text: StdString = (validated,).apply(&func).unwrap();
            assert_eq!(text, "foobar");
        });
    }

    #[test]
    fn rope_string() {
        test_with(|ctx| {
            let val: Value = ctx
                .eval(
                    r#"
                    let s = "";
                    for (let i = 0; i < 10000; i++) s += "Line " + i + "\n";
                    s
                "#,
                )
                .unwrap();

            assert_eq!(val.type_of(), Type::String);
            assert!(val.is_string());
            assert!(val.as_string().is_some());

            let s: StdString = val.as_string().unwrap().to_string().unwrap();
            assert!(s.starts_with("Line 0\n"));
            assert!(s.contains("Line 999\n"));
            assert!(s.len() > 8000);
        });
    }
}
