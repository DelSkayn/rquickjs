use crate::{
    atom::PredefinedAtom, qjs, ArrayBuffer, Ctx, Error, FromJs, IntoJs, JsLifetime, Object, Result,
    Value,
};
use std::{
    fmt,
    marker::PhantomData,
    mem::{self, MaybeUninit},
    ops::Deref,
    ptr::{null_mut, NonNull},
    slice,
};

use super::array_buffer::RawArrayBuffer;

/// The trait which implements types which capable to be TypedArray items
///
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "array-buffer")))]
pub trait TypedArrayItem: Copy {
    const CLASS_NAME: PredefinedAtom;
    const ARRAY_TYPE: qjs::JSTypedArrayEnum;
}

macro_rules! typedarray_items {
    ($($name:ident: $type:ty, $array_type:expr,)*) => {
        $(impl TypedArrayItem for $type {
            const CLASS_NAME: PredefinedAtom = PredefinedAtom::$name;
            const ARRAY_TYPE: qjs::JSTypedArrayEnum = $array_type;
        })*
    };
}

typedarray_items! {
    Int8Array: i8, qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_INT8,
    Uint8Array: u8, qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_UINT8,
    Int16Array: i16, qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_INT16,
    Uint16Array: u16, qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_UINT16,
    Int32Array: i32, qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_INT32,
    Uint32Array: u32, qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_UINT32,
// XXX introduce when f16 is
// Float16Array: f16, qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_FLOAT16,
    Float32Array: f32, qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_FLOAT32,
    Float64Array: f64, qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_FLOAT64,
    BigInt64Array: i64, qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_BIG_INT64,
    BigUint64Array: u64, qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_BIG_UINT64,
}

/// Rust representation of a JavaScript objects of TypedArray classes.
///
/// | ES Type            | Rust Type             |
/// | ------------------ | --------------------- |
/// | `Int8Array`        | [`TypedArray<i8>`]    |
/// | `Uint8Array`       | [`TypedArray<u8>`]    |
/// | `Int16Array`       | [`TypedArray<i16>`]   |
/// | `Uint16Array`      | [`TypedArray<u16>`]   |
/// | `Int32Array`       | [`TypedArray<i32>`]   |
/// | `Uint32Array`      | [`TypedArray<u32>`]   |
/// | `Float32Array`     | [`TypedArray<f32>`]   |
/// | `Float64Array`     | [`TypedArray<f64>`]   |
/// | `BigInt64Array`    | [`TypedArray<i64>`]   |
/// | `BigUint64Array`   | [`TypedArray<u64>`]   |
///
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "array-buffer")))]
#[repr(transparent)]
pub struct TypedArray<'js, T>(pub(crate) Object<'js>, PhantomData<T>);

unsafe impl<'js, T: JsLifetime<'js>> JsLifetime<'js> for TypedArray<'js, T> {
    type Changed<'to> = TypedArray<'to, T::Changed<'to>>;
}

impl<'js, T> fmt::Debug for TypedArray<'js, T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_tuple("TypedArray").field(&self.0).finish()
    }
}

impl<'js, T> PartialEq for TypedArray<'js, T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<'js, T> Clone for TypedArray<'js, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<'js, T> TypedArray<'js, T> {
    /// Create typed array from vector data
    pub fn new(ctx: Ctx<'js>, src: impl Into<Vec<T>>) -> Result<Self>
    where
        T: Copy + TypedArrayItem,
    {
        let ab = ArrayBuffer::new(ctx, src)?;
        Self::from_arraybuffer(ab)
    }

    /// Create typed array from slice
    pub fn new_copy(ctx: Ctx<'js>, src: impl AsRef<[T]>) -> Result<Self>
    where
        T: Copy + TypedArrayItem,
    {
        let ab = ArrayBuffer::new_copy(ctx, src)?;
        Self::from_arraybuffer(ab)
    }

    /// Get the length of the typed array in elements.
    pub fn len(&self) -> usize {
        //Self::get_raw(&self.0).expect("Not a TypedArray").0
        let ctx = &self.0.ctx;
        let value = self.0.as_js_value();
        unsafe {
            let val = qjs::JS_GetProperty(ctx.as_ptr(), value, PredefinedAtom::Length as _);
            assert!(qjs::JS_IsInt(val));
            qjs::JS_VALUE_GET_INT(val) as _
        }
    }

    /// Returns whether a typed array is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Reference to value
    #[inline]
    pub fn as_value(&self) -> &Value<'js> {
        self.0.as_value()
    }

    /// Convert into value
    #[inline]
    pub fn into_value(self) -> Value<'js> {
        self.0.into_value()
    }

    /// Convert from value
    pub fn from_value(value: Value<'js>) -> Result<Self>
    where
        T: TypedArrayItem,
    {
        Self::from_object(Object::from_value(value)?)
    }

    /// Reference as an object
    #[inline]
    pub fn as_object(&self) -> &Object<'js> {
        &self.0
    }

    /// Convert into an object
    #[inline]
    pub fn into_object(self) -> Object<'js> {
        self.0
    }

    /// Convert from an object
    pub fn from_object(object: Object<'js>) -> Result<Self>
    where
        T: TypedArrayItem,
    {
        if object.is_typed_array::<T>() {
            return Ok(Self(object, PhantomData));
        }
        Err(Error::new_from_js("object", T::CLASS_NAME.to_str()))
    }

    /// Returns the underlying bytes of the buffer,
    ///
    /// Returns `None` if the array is detached.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        let (_, len, ptr) = Self::get_raw_bytes(self.as_value())?;
        Some(unsafe { slice::from_raw_parts(ptr.as_ptr(), len) })
    }

    pub fn as_raw(&self) -> Option<RawArrayBuffer> {
        let (_, len, ptr) = Self::get_raw_bytes(self.as_value())?;
        Some(RawArrayBuffer { len, ptr })
    }

    /// Get underlying ArrayBuffer
    pub fn arraybuffer(&self) -> Result<ArrayBuffer<'js>> {
        let ctx = self.ctx().clone();
        let val = self.0.as_js_value();
        let buf = unsafe {
            let val =
                qjs::JS_GetTypedArrayBuffer(ctx.as_ptr(), val, null_mut(), null_mut(), null_mut());
            ctx.handle_exception(val)?;
            Value::from_js_value(ctx.clone(), val)
        };
        ArrayBuffer::from_js(&ctx, buf)
    }

    /// Convert from an ArrayBuffer
    pub fn from_arraybuffer(arraybuffer: ArrayBuffer<'js>) -> Result<Self>
    where
        T: TypedArrayItem,
    {
        let ctx = &arraybuffer.0.ctx;

        let val = unsafe {
            let mut argv: [qjs::JSValue; 3] =
                [arraybuffer.0.as_raw(), qjs::JS_UNDEFINED, qjs::JS_UNDEFINED];
            let val = qjs::JS_NewTypedArray(
                ctx.as_ptr(),
                argv.len() as i32,
                argv.as_mut_ptr(),
                T::ARRAY_TYPE,
            );
            ctx.handle_exception(val)?;
            Value::from_js_value(ctx.clone(), val)
        };
        Ok(Self(Object(val), PhantomData))
    }

    pub(crate) fn get_raw_bytes(val: &Value<'js>) -> Option<(usize, usize, NonNull<u8>)> {
        let ctx = &val.ctx;
        let val = val.as_js_value();
        let mut off = MaybeUninit::<qjs::size_t>::uninit();
        let mut len = MaybeUninit::<qjs::size_t>::uninit();
        let mut stp = MaybeUninit::<qjs::size_t>::uninit();
        let buf = unsafe {
            let val = qjs::JS_GetTypedArrayBuffer(
                ctx.as_ptr(),
                val,
                off.as_mut_ptr(),
                len.as_mut_ptr(),
                stp.as_mut_ptr(),
            );
            ctx.handle_exception(val).ok()?;
            Value::from_js_value(ctx.clone(), val)
        };
        let off: usize = unsafe { off.assume_init() }
            .try_into()
            .expect(qjs::SIZE_T_ERROR);
        let len: usize = unsafe { len.assume_init() }
            .try_into()
            .expect(qjs::SIZE_T_ERROR);
        let stp: usize = unsafe { stp.assume_init() }
            .try_into()
            .expect(qjs::SIZE_T_ERROR);
        let raw = ArrayBuffer::get_raw(&buf)?;
        if (off + len) > raw.len {
            return None;
        }
        // SAFETY: ptr was non-null and then we added an offset so it should still be non null
        let ptr = unsafe { NonNull::new_unchecked(raw.ptr.as_ptr().add(off)) };
        Some((stp, len, ptr))
    }

    pub(crate) fn get_raw(val: &Value<'js>) -> Option<(usize, NonNull<T>)> {
        let (stp, len, ptr) = Self::get_raw_bytes(val)?;
        if stp != mem::size_of::<T>() {
            return None;
        }
        debug_assert_eq!(ptr.as_ptr().align_offset(mem::align_of::<T>()), 0);
        let ptr = ptr.cast::<T>();
        Some((len / mem::size_of::<T>(), ptr))
    }
}

impl<'js, T: TypedArrayItem> AsRef<[T]> for TypedArray<'js, T> {
    fn as_ref(&self) -> &[T] {
        let (len, ptr) =
            Self::get_raw(&self.0).unwrap_or_else(|| panic!("{}", T::CLASS_NAME.to_str()));
        unsafe { slice::from_raw_parts(ptr.as_ptr().cast(), len) }
    }
}

impl<'js, T> Deref for TypedArray<'js, T> {
    type Target = Object<'js>;

    fn deref(&self) -> &Self::Target {
        self.as_object()
    }
}

impl<'js, T> AsRef<Object<'js>> for TypedArray<'js, T> {
    fn as_ref(&self) -> &Object<'js> {
        self.as_object()
    }
}

impl<'js, T> AsRef<Value<'js>> for TypedArray<'js, T> {
    fn as_ref(&self) -> &Value<'js> {
        self.as_value()
    }
}

impl<'js, T> TryFrom<TypedArray<'js, T>> for ArrayBuffer<'js> {
    type Error = Error;

    fn try_from(ta: TypedArray<'js, T>) -> Result<Self> {
        ta.arraybuffer()
    }
}

impl<'js, T> TryFrom<ArrayBuffer<'js>> for TypedArray<'js, T>
where
    T: TypedArrayItem,
{
    type Error = Error;

    fn try_from(ab: ArrayBuffer<'js>) -> Result<Self> {
        Self::from_arraybuffer(ab)
    }
}

impl<'js, T> FromJs<'js> for TypedArray<'js, T>
where
    T: TypedArrayItem,
{
    fn from_js(_: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Self::from_value(value)
    }
}

impl<'js, T> IntoJs<'js> for TypedArray<'js, T> {
    fn into_js(self, _: &Ctx<'js>) -> Result<Value<'js>> {
        Ok(self.into_value())
    }
}

impl<'js> Object<'js> {
    pub fn is_typed_array<T: TypedArrayItem>(&self) -> bool {
        let array_type = unsafe { qjs::JS_GetTypedArrayType(self.value) };
        T::ARRAY_TYPE == array_type as u32
    }

    /// Interpret as [`TypedArray`]
    ///
    /// # Safety
    /// You should be sure that the object actually is the required type.
    pub unsafe fn ref_typed_array<'a, T: TypedArrayItem>(&'a self) -> &'a TypedArray<'js, T> {
        mem::transmute(self)
    }

    pub fn as_typed_array<'a, T: TypedArrayItem>(&self) -> Option<&TypedArray<'js, T>> {
        self.is_typed_array::<T>()
            .then(|| unsafe { self.ref_typed_array() })
    }
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn from_javascript_i8() {
        test_with(|ctx| {
            let val: TypedArray<i8> = ctx
                .eval(
                    r#"
                        new Int8Array([0, -5, 1, 11])
                    "#,
                )
                .unwrap();
            assert_eq!(val.len(), 4);
            assert_eq!(val.as_ref() as &[i8], &[0i8, -5, 1, 11]);
        });
    }

    #[test]
    fn into_javascript_i8() {
        test_with(|ctx| {
            let val = TypedArray::<i8>::new(ctx.clone(), [-1i8, 0, 22, 5]).unwrap();
            ctx.globals().set("v", val).unwrap();
            let res: i8 = ctx
                .eval(
                    r#"
                    v.length
                        v.length != 4 ? 1 :
                        v[0] != -1 ? 2 :
                        v[1] != 0 ? 3 :
                        v[2] != 22 ? 4 :
                        v[3] != 5 ? 5 :
                        0
                    "#,
                )
                .unwrap();
            assert_eq!(res, 0);
        })
    }

    #[test]
    fn from_javascript_f32() {
        test_with(|ctx| {
            let val: TypedArray<f32> = ctx
                .eval(
                    r#"
                        new Float32Array([0.5, -5.25, 123.125])
                    "#,
                )
                .unwrap();
            assert_eq!(val.len(), 3);
            assert_eq!(val.as_ref() as &[f32], &[0.5, -5.25, 123.125]);
        });
    }

    #[test]
    fn into_javascript_f32() {
        test_with(|ctx| {
            let val = TypedArray::<f32>::new(ctx.clone(), [-1.5, 0.0, 2.25]).unwrap();
            ctx.globals().set("v", val).unwrap();
            let res: i8 = ctx
                .eval(
                    r#"
                        v.length != 3 ? 1 :
                        v[0] != -1.5 ? 2 :
                        v[1] != 0 ? 3 :
                        v[2] != 2.25 ? 4 :
                        0
                    "#,
                )
                .unwrap();
            assert_eq!(res, 0);
        })
    }

    #[test]
    fn as_bytes() {
        test_with(|ctx| {
            let val: TypedArray<u32> = ctx
                .eval(
                    r#"
                        new Uint32Array([0xCAFEDEAD,0xFEEDBEAD])
                    "#,
                )
                .unwrap();
            let mut res = [0; 8];
            let bytes_0 = 0xCAFEDEADu32.to_ne_bytes();
            res[..4].copy_from_slice(&bytes_0);
            let bytes_1 = 0xFEEDBEADu32.to_ne_bytes();
            res[4..].copy_from_slice(&bytes_1);

            assert_eq!(val.as_bytes().unwrap(), &res)
        });
    }
}
