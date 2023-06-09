use crate::{
    qjs, ArrayBuffer, Ctx, Error, FromJs, Function, IntoJs, Object, Outlive, Result, Value,
};
use std::{
    convert::TryFrom,
    marker::PhantomData,
    mem::{size_of, MaybeUninit},
    ops::Deref,
    ptr::null_mut,
    slice,
};

/// The trait which implements types which capable to be TypedArray items
///
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "array-buffer")))]
pub trait TypedArrayItem {
    const CLASS_NAME: &'static str;
}

macro_rules! typedarray_items {
    ($($name:ident: $type:ty,)*) => {
        $(impl TypedArrayItem for $type {
            const CLASS_NAME: &'static str = stringify!($name);
        })*
    };
}

typedarray_items! {
    Int8Array: i8,
    Uint8Array: u8,
    Int16Array: i16,
    Uint16Array: u16,
    Int32Array: i32,
    Uint32Array: u32,
    Float32Array: f32,
    Float64Array: f64,
    BigInt64Array: i64,
    BigUint64Array: u64,
}

/// Rust representation of a javascript objects of TypedArray classes.
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
#[derive(Debug, PartialEq, Clone)]
#[repr(transparent)]
pub struct TypedArray<'js, T>(pub(crate) Object<'js>, PhantomData<T>);

impl<'js, 't, T> Outlive<'t> for TypedArray<'js, T> {
    type Target = TypedArray<'t, T>;
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
        let ctx = self.0.ctx;
        let value = self.0.as_js_value();
        unsafe {
            let val = qjs::JS_GetPropertyStr(ctx.as_ptr(), value, b"length\0".as_ptr() as *const _);
            assert!(qjs::JS_IsInt(val));
            qjs::JS_VALUE_GET_INT(val) as _
        }
    }

    /// Returns wether a typed array is empty.
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
        let class: Function = object.ctx.globals().get(T::CLASS_NAME)?;
        if object.is_instance_of(class) {
            Ok(Self(object, PhantomData))
        } else {
            Err(Error::new_from_js("object", T::CLASS_NAME))
        }
    }

    /// Returns the underlying bytes of the buffer,
    ///
    /// Returns None if the array is detached.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        let mut len = MaybeUninit::<usize>::uninit();
        unsafe {
            let ptr =
                qjs::JS_GetArrayBuffer(self.0.ctx.as_ptr(), len.as_mut_ptr(), self.0.as_js_value());
            if ptr.is_null() {
                return None;
            }
            let len = len.assume_init();
            Some(slice::from_raw_parts::<u8>(ptr, len))
        }
    }

    /// Get underlaying ArrayBuffer
    pub fn arraybuffer(&self) -> Result<ArrayBuffer<'js>> {
        let ctx = self.0.ctx;
        let val = self.0.as_js_value();
        let buf = unsafe {
            let val =
                qjs::JS_GetTypedArrayBuffer(ctx.as_ptr(), val, null_mut(), null_mut(), null_mut());
            ctx.handle_exception(val)?;
            Value::from_js_value(ctx, val)
        };
        ArrayBuffer::from_value(buf)
    }

    /// Convert from an ArrayBuffer
    pub fn from_arraybuffer(arraybuffer: ArrayBuffer<'js>) -> Result<Self>
    where
        T: TypedArrayItem,
    {
        let ctx = arraybuffer.0.ctx;
        let ctor: Function = ctx.globals().get(T::CLASS_NAME)?;
        ctor.construct((arraybuffer,))
    }

    pub(crate) fn get_raw(val: &Value<'js>) -> Option<(usize, *mut T)> {
        let ctx = val.ctx;
        let val = val.as_js_value();
        let mut off = MaybeUninit::<usize>::uninit();
        let mut len = MaybeUninit::<usize>::uninit();
        let mut stp = MaybeUninit::<usize>::uninit();
        let buf = unsafe {
            let val = qjs::JS_GetTypedArrayBuffer(
                ctx.as_ptr(),
                val,
                off.as_mut_ptr(),
                len.as_mut_ptr(),
                stp.as_mut_ptr(),
            );
            ctx.handle_exception(val).ok()?;
            Value::from_js_value(ctx, val)
        };
        let off = unsafe { off.assume_init() };
        let len = unsafe { len.assume_init() };
        let stp = unsafe { stp.assume_init() };
        if stp != size_of::<T>() {
            return None;
        }
        let (full_len, ptr) = ArrayBuffer::get_raw(&buf)?;
        if (off + len) > full_len {
            return None;
        }
        let len = len / size_of::<T>();
        let ptr = unsafe { ptr.add(off) } as *mut T;
        Some((len, ptr))
    }
}

impl<'js, T: TypedArrayItem> AsRef<[T]> for TypedArray<'js, T> {
    fn as_ref(&self) -> &[T] {
        let (len, ptr) = Self::get_raw(&self.0).expect(T::CLASS_NAME);
        unsafe { slice::from_raw_parts(ptr as _, len) }
    }
}

impl<'js, T: TypedArrayItem> AsMut<[T]> for TypedArray<'js, T> {
    fn as_mut(&mut self) -> &mut [T] {
        let (len, ptr) = Self::get_raw(&self.0).expect(T::CLASS_NAME);
        unsafe { slice::from_raw_parts_mut(ptr, len) }
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
    fn from_js(_: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Self::from_value(value)
    }
}

impl<'js, T> IntoJs<'js> for TypedArray<'js, T> {
    fn into_js(self, _: Ctx<'js>) -> Result<Value<'js>> {
        Ok(self.into_value())
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
            let val = TypedArray::<i8>::new(ctx, [-1i8, 0, 22, 5]).unwrap();
            ctx.globals().set("v", val).unwrap();
            let res: i8 = ctx
                .eval(
                    r#"
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
            let val = TypedArray::<f32>::new(ctx, [-1.5, 0.0, 2.25]).unwrap();
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
}
