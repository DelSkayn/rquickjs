use crate::{qjs, Ctx, Error, FromJs, IntoJs, Object, Outlive, Result, Value};
use std::{
    convert::TryInto,
    mem::{self, size_of, ManuallyDrop, MaybeUninit},
    ops::Deref,
    os::raw::c_void,
    slice,
};

use super::typed_array::TypedArrayItem;

/// Rust representation of a javascript object of class ArrayBuffer.
///
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "array-buffer")))]
#[derive(Debug, PartialEq, Clone)]
#[repr(transparent)]
pub struct ArrayBuffer<'js>(pub(crate) Object<'js>);

unsafe impl<'js, 't> Outlive<'t> for ArrayBuffer<'js> {
    type Target = ArrayBuffer<'t>;
}

impl<'js> ArrayBuffer<'js> {
    /// Create array buffer from vector data
    pub fn new<T: Copy>(ctx: Ctx<'js>, src: impl Into<Vec<T>>) -> Result<Self> {
        let mut src = ManuallyDrop::new(src.into());
        let ptr = src.as_mut_ptr();
        let capacity = src.capacity();
        let size = src.len() * size_of::<T>();

        extern "C" fn drop_raw<T>(_rt: *mut qjs::JSRuntime, opaque: *mut c_void, ptr: *mut c_void) {
            let ptr = ptr as *mut T;
            let capacity = opaque as usize;
            // reconstruct vector in order to free data
            // the length of actual data does not matter for copyable types
            unsafe { Vec::from_raw_parts(ptr, capacity, capacity) };
        }

        Ok(Self(Object(unsafe {
            let val = qjs::JS_NewArrayBuffer(
                ctx.as_ptr(),
                ptr as _,
                size as _,
                Some(drop_raw::<T>),
                capacity as _,
                0,
            );
            ctx.handle_exception(val).map_err(|error| {
                // don't forget to free data when error occurred
                Vec::from_raw_parts(ptr, capacity, capacity);
                error
            })?;
            Value::from_js_value(ctx, val)
        })))
    }

    /// Create array buffer from slice
    pub fn new_copy<T: Copy>(ctx: Ctx<'js>, src: impl AsRef<[T]>) -> Result<Self> {
        let src = src.as_ref();
        let ptr = src.as_ptr();
        let size = std::mem::size_of_val(src);

        Ok(Self(Object(unsafe {
            let val = qjs::JS_NewArrayBufferCopy(ctx.as_ptr(), ptr as _, size as _);
            ctx.handle_exception(val)?;
            Value::from_js_value(ctx, val)
        })))
    }

    /// Get the length of the array buffer in bytes.
    pub fn len(&self) -> usize {
        Self::get_raw(&self.0).expect("Not an ArrayBuffer").0
    }

    /// Returns wether an array buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the underlying bytes of the buffer,
    ///
    /// Returns None if the array is detached.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        let (len, ptr) = Self::get_raw(self.as_value())?;
        Some(unsafe { slice::from_raw_parts_mut(ptr, len) })
    }

    /// Returns a slice if the buffer underlying buffer is properly aligned for the type and the
    /// buffer is not detached.
    pub fn as_slice<T: TypedArrayItem>(&self) -> Option<&[T]> {
        let (len, ptr) = Self::get_raw(&self.0)?;
        if ptr.align_offset(mem::align_of::<T>()) != 0 {
            return None;
        }
        //assert!(len % size_of::<T>() == 0);
        let len = len / size_of::<T>();
        Some(unsafe { slice::from_raw_parts(ptr as _, len) })
    }

    /// Detach array buffer
    pub fn detach(&mut self) {
        unsafe { qjs::JS_DetachArrayBuffer(self.0.ctx.as_ptr(), self.0.as_js_value()) }
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
    pub fn from_value(value: Value<'js>) -> Result<Self> {
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
    pub fn from_object(object: Object<'js>) -> Result<Self> {
        if Self::get_raw(&object.0).is_some() {
            Ok(Self(object))
        } else {
            Err(Error::new_from_js("object", "ArrayBuffer"))
        }
    }

    pub(crate) fn get_raw(val: &Value<'js>) -> Option<(usize, *mut u8)> {
        let ctx = val.ctx;
        let val = val.as_js_value();
        let mut size = MaybeUninit::<qjs::size_t>::uninit();
        let ptr = unsafe { qjs::JS_GetArrayBuffer(ctx.as_ptr(), size.as_mut_ptr(), val) };

        if ptr.is_null() {
            None
        } else {
            let len = unsafe { size.assume_init() }
                .try_into()
                .expect(qjs::SIZE_T_ERROR);
            Some((len, ptr))
        }
    }
}

impl<'js, T: TypedArrayItem> AsRef<[T]> for ArrayBuffer<'js> {
    fn as_ref(&self) -> &[T] {
        self.as_slice().expect("Arraybuffer was detached")
    }
}

impl<'js> Deref for ArrayBuffer<'js> {
    type Target = Object<'js>;

    fn deref(&self) -> &Self::Target {
        self.as_object()
    }
}

impl<'js> AsRef<Object<'js>> for ArrayBuffer<'js> {
    fn as_ref(&self) -> &Object<'js> {
        self.as_object()
    }
}

impl<'js> AsRef<Value<'js>> for ArrayBuffer<'js> {
    fn as_ref(&self) -> &Value<'js> {
        self.as_value()
    }
}

impl<'js> FromJs<'js> for ArrayBuffer<'js> {
    fn from_js(_: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Self::from_value(value)
    }
}

impl<'js> IntoJs<'js> for ArrayBuffer<'js> {
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
            let val: ArrayBuffer = ctx
                .eval(
                    r#"
                        new Int8Array([0, -5, 1, 11]).buffer
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
            let val = ArrayBuffer::new(ctx, [-1i8, 0, 22, 5]).unwrap();
            ctx.globals().set("a", val).unwrap();
            let res: i8 = ctx
                .eval(
                    r#"
                        let v = new Int8Array(a);
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
            let val: ArrayBuffer = ctx
                .eval(
                    r#"
                        new Float32Array([0.5, -5.25, 123.125]).buffer
                    "#,
                )
                .unwrap();
            assert_eq!(val.len(), 12);
            assert_eq!(val.as_ref() as &[f32], &[0.5f32, -5.25, 123.125]);
        });
    }

    #[test]
    fn into_javascript_f32() {
        test_with(|ctx| {
            let val = ArrayBuffer::new(ctx, [-1.5f32, 0.0, 2.25]).unwrap();
            ctx.globals().set("a", val).unwrap();
            let res: i8 = ctx
                .eval(
                    r#"
                        let v = new Float32Array(a);
                        a.byteLength != 12 ? 1 :
                        v.length != 3 ? 2 :
                        v[0] != -1.5 ? 3 :
                        v[1] != 0 ? 4 :
                        v[2] != 2.25 ? 5 :
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
            let val: ArrayBuffer = ctx
                .eval(
                    r#"
                        new Uint32Array([0xCAFEDEAD,0xFEEDBEAD]).buffer
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
