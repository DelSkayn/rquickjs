use crate::{qjs, Ctx, Error, FromJs, IntoJs, JsLifetime, Object, Result, Value};
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::{
    ffi::c_void,
    fmt,
    mem::{self, size_of, ManuallyDrop, MaybeUninit},
    ops::Deref,
    ptr::NonNull,
    result::Result as StdResult,
    slice,
};

use super::typed_array::TypedArrayItem;

/// The drop callback invoked when an externally backed `ArrayBuffer` is
/// garbage-collected (or when construction fails).
#[cfg(not(feature = "parallel"))]
pub type ArrayBufferDrop = Box<dyn FnOnce() + 'static>;
/// The drop callback invoked when an externally backed `ArrayBuffer` is
/// garbage-collected (or when construction fails).
#[cfg(feature = "parallel")]
pub type ArrayBufferDrop = Box<dyn FnOnce() + Send + 'static>;

/// Marker bound used by the `from_source*` APIs so that the captured source
/// satisfies the `Send` requirement only when the `parallel` feature is
/// enabled.
#[cfg(not(feature = "parallel"))]
pub trait DropSend {}
#[cfg(not(feature = "parallel"))]
impl<T> DropSend for T {}
#[cfg(feature = "parallel")]
pub trait DropSend: Send {}
#[cfg(feature = "parallel")]
impl<T: Send> DropSend for T {}

/// A contiguous byte region owned by `self` and usable as the backing store
/// of an [`ArrayBuffer`].
///
/// # Safety
///
/// * `as_ptr()` must return a pointer that is valid for reads (and writes,
///   when used via [`ArrayBuffer::from_source`] / [`ArrayBuffer::from_source_shared`])
///   of `len()` bytes.
/// * The returned pointer must remain valid until `self` is dropped, including
///   across moves of `self`.
pub unsafe trait ArrayBufferSource {
    fn as_ptr(&self) -> *mut u8;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

macro_rules! impl_array_buffer_source {
    ($($t:ty),* $(,)?) => {
        $(
            unsafe impl ArrayBufferSource for $t {
                fn as_ptr(&self) -> *mut u8 {
                    <[u8]>::as_ptr(self) as *mut u8
                }
                fn len(&self) -> usize {
                    <[u8]>::len(self)
                }
            }
        )*
    };
}

impl_array_buffer_source!(Vec<u8>, alloc::boxed::Box<[u8]>, Arc<[u8]>, Arc<Vec<u8>>);

#[cfg(feature = "bytes")]
impl_array_buffer_source!(bytes::Bytes);

pub struct RawArrayBuffer {
    pub len: usize,
    pub ptr: NonNull<u8>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AsSliceError {
    BufferUsed,
    InvalidAlignment,
}

impl fmt::Display for AsSliceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AsSliceError::BufferUsed => write!(f, "Buffer was already used"),
            AsSliceError::InvalidAlignment => {
                write!(f, "Buffer had a different alignment than was requested")
            }
        }
    }
}

/// Rust representation of a JavaScript object of class ArrayBuffer.
///
#[derive(Debug, PartialEq, Clone, Eq, Hash)]
#[repr(transparent)]
pub struct ArrayBuffer<'js>(pub(crate) Object<'js>);

unsafe impl<'js> JsLifetime<'js> for ArrayBuffer<'js> {
    type Changed<'to> = ArrayBuffer<'to>;
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
                false,
            );
            ctx.handle_exception(val).inspect_err(|_| {
                // don't forget to free data when error occurred
                Vec::from_raw_parts(ptr, capacity, capacity);
            })?;
            Value::from_js_value(ctx, val)
        })))
    }

    /// Create array buffer from slice
    pub fn new_copy<T: Copy>(ctx: Ctx<'js>, src: impl AsRef<[T]>) -> Result<Self> {
        let src = src.as_ref();
        let ptr = src.as_ptr();
        let size = core::mem::size_of_val(src);

        Ok(Self(Object(unsafe {
            let val = qjs::JS_NewArrayBufferCopy(ctx.as_ptr(), ptr as _, size as _);
            ctx.handle_exception(val)?;
            Value::from_js_value(ctx.clone(), val)
        })))
    }

    /// Create an `ArrayBuffer` backed by an external buffer.
    ///
    /// `drop` is invoked exactly once: when the buffer is garbage-collected,
    /// or synchronously if construction fails. It should release whatever
    /// backing storage it captures.
    ///
    /// # Safety
    ///
    /// `ptr` must point to `len` bytes of valid memory for the entire lifetime
    /// of the returned buffer and any views of it. The `drop` closure must
    /// release that memory correctly.
    pub unsafe fn from_raw_parts(
        ctx: Ctx<'js>,
        ptr: *mut u8,
        len: usize,
        drop: ArrayBufferDrop,
    ) -> Result<Self> {
        unsafe { Self::from_raw_parts_inner(ctx, ptr, len, drop, false, false) }
    }

    /// Create a JS `SharedArrayBuffer` backed by an external buffer.
    ///
    /// Same semantics as [`from_raw_parts`] but produces a JS `SharedArrayBuffer`,
    /// which supports `Atomics` and can be shared across workers.
    ///
    /// # Safety
    ///
    /// See [`from_raw_parts`].
    pub unsafe fn from_raw_parts_shared(
        ctx: Ctx<'js>,
        ptr: *mut u8,
        len: usize,
        drop: ArrayBufferDrop,
    ) -> Result<Self> {
        unsafe { Self::from_raw_parts_inner(ctx, ptr, len, drop, true, false) }
    }

    /// Create an `ArrayBuffer` that JS sees as immutable.
    ///
    /// Same semantics as [`from_raw_parts`] but any JS-side write throws,
    /// which makes it sound to back the buffer with a shared-immutable
    /// Rust value (`Arc<[u8]>`, `bytes::Bytes`, …) that is still accessible
    /// from Rust as `&[u8]` after this call returns.
    ///
    /// # Safety
    ///
    /// See [`from_raw_parts`].
    pub unsafe fn from_raw_parts_immutable(
        ctx: Ctx<'js>,
        ptr: *mut u8,
        len: usize,
        drop: ArrayBufferDrop,
    ) -> Result<Self> {
        unsafe { Self::from_raw_parts_inner(ctx, ptr, len, drop, false, true) }
    }

    unsafe fn from_raw_parts_inner(
        ctx: Ctx<'js>,
        ptr: *mut u8,
        len: usize,
        drop: ArrayBufferDrop,
        is_shared: bool,
        immutable: bool,
    ) -> Result<Self> {
        extern "C" fn shim(_rt: *mut qjs::JSRuntime, opaque: *mut c_void, _ptr: *mut c_void) {
            unsafe {
                let boxed: Box<ArrayBufferDrop> = Box::from_raw(opaque as *mut ArrayBufferDrop);
                (*boxed)();
            }
        }

        let opaque = Box::into_raw(Box::new(drop)) as *mut c_void;

        Ok(Self(Object(unsafe {
            let val =
                qjs::JS_NewArrayBuffer(ctx.as_ptr(), ptr, len as _, Some(shim), opaque, is_shared);
            if let Err(e) = ctx.handle_exception(val) {
                shim(qjs::JS_GetRuntime(ctx.as_ptr()), opaque, ptr as *mut c_void);
                return Err(e);
            }
            if immutable {
                qjs::JS_SetImmutableArrayBuffer(val, true);
            }
            Value::from_js_value(ctx, val)
        })))
    }

    /// Create an `ArrayBuffer` from a source that owns its backing bytes.
    ///
    /// The source is moved into the buffer; JS has exclusive mutable access
    /// until the buffer is collected, at which point the source is dropped.
    /// Using this with a shared-immutable source (`Arc<[u8]>`, `bytes::Bytes`,
    /// …) is unsound; use [`from_source_immutable`] for those.
    pub fn from_source<S>(ctx: Ctx<'js>, src: S) -> Result<Self>
    where
        S: ArrayBufferSource + DropSend + 'static,
    {
        let ptr = src.as_ptr();
        let len = src.len();
        unsafe { Self::from_raw_parts(ctx, ptr, len, Box::new(move || drop(src))) }
    }

    /// Create a `SharedArrayBuffer` from a source that owns its backing bytes.
    ///
    /// See [`from_source`] for ownership semantics.
    pub fn from_source_shared<S>(ctx: Ctx<'js>, src: S) -> Result<Self>
    where
        S: ArrayBufferSource + DropSend + 'static,
    {
        let ptr = src.as_ptr();
        let len = src.len();
        unsafe { Self::from_raw_parts_shared(ctx, ptr, len, Box::new(move || drop(src))) }
    }

    /// Create an `ArrayBuffer` that JS sees as immutable, backed by a
    /// possibly shared-immutable source.
    ///
    /// JS writes throw, so it is sound for the caller to keep holding
    /// shared-immutable references to the backing store (`Arc<[u8]>`,
    /// `bytes::Bytes`, …).
    pub fn from_source_immutable<S>(ctx: Ctx<'js>, src: S) -> Result<Self>
    where
        S: ArrayBufferSource + DropSend + 'static,
    {
        let ptr = src.as_ptr();
        let len = src.len();
        unsafe { Self::from_raw_parts_immutable(ctx, ptr, len, Box::new(move || drop(src))) }
    }

    /// Get the length of the array buffer in bytes.
    pub fn len(&self) -> usize {
        Self::get_raw(&self.0).expect("Not an ArrayBuffer").len
    }

    /// Returns whether an array buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the underlying bytes of the buffer,
    ///
    /// Returns `None` if the array is detached.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        let raw = Self::get_raw(self.as_value())?;
        Some(unsafe { slice::from_raw_parts_mut(raw.ptr.as_ptr(), raw.len) })
    }

    /// Returns a slice if the buffer underlying buffer is properly aligned for the type and the
    /// buffer is not detached.
    pub fn as_slice<T: TypedArrayItem>(&self) -> StdResult<&[T], AsSliceError> {
        let raw = Self::get_raw(&self.0).ok_or(AsSliceError::BufferUsed)?;
        if raw.ptr.as_ptr().align_offset(mem::align_of::<T>()) != 0 {
            return Err(AsSliceError::InvalidAlignment);
        }
        let len = raw.len / size_of::<T>();
        Ok(unsafe { slice::from_raw_parts(raw.ptr.as_ptr().cast(), len) })
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
    pub fn from_value(value: Value<'js>) -> Option<Self> {
        Self::from_object(Object::from_value(value).ok()?)
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
    pub fn from_object(object: Object<'js>) -> Option<Self> {
        if Self::get_raw(&object.0).is_some() {
            Some(Self(object))
        } else {
            None
        }
    }

    /// Returns a structure with data about the raw buffer which this object contains.
    ///
    /// Returns None if the buffer was already used.
    pub fn as_raw(&self) -> Option<RawArrayBuffer> {
        Self::get_raw(self.as_value())
    }

    pub(crate) fn get_raw(val: &Value<'js>) -> Option<RawArrayBuffer> {
        let ctx = val.ctx();
        let val = val.as_js_value();
        let mut size = MaybeUninit::<qjs::size_t>::uninit();
        let ptr = unsafe { qjs::JS_GetArrayBuffer(ctx.as_ptr(), size.as_mut_ptr(), val) };

        if let Some(ptr) = NonNull::new(ptr) {
            let len = unsafe { size.assume_init() }
                .try_into()
                .expect(qjs::SIZE_T_ERROR);
            Some(RawArrayBuffer { len, ptr })
        } else {
            None
        }
    }
}

impl<'js, T: TypedArrayItem> AsRef<[T]> for ArrayBuffer<'js> {
    fn as_ref(&self) -> &[T] {
        self.as_slice().expect("ArrayBuffer was detached")
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
    fn from_js(_: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let ty_name = value.type_name();
        if let Some(v) = Self::from_value(value) {
            Ok(v)
        } else {
            Err(Error::new_from_js(ty_name, "ArrayBuffer"))
        }
    }
}

impl<'js> IntoJs<'js> for ArrayBuffer<'js> {
    fn into_js(self, _: &Ctx<'js>) -> Result<Value<'js>> {
        Ok(self.into_value())
    }
}

impl<'js> Object<'js> {
    /// Returns whether the object is an instance of [`ArrayBuffer`].
    pub fn is_array_buffer(&self) -> bool {
        ArrayBuffer::get_raw(&self.0).is_some()
    }

    /// Interpret as [`ArrayBuffer`]
    ///
    /// # Safety
    /// You should be sure that the object actually is the required type.
    pub unsafe fn ref_array_buffer(&self) -> &ArrayBuffer {
        mem::transmute(self)
    }

    /// Turn the object into an array buffer if the object is an instance of [`ArrayBuffer`].
    pub fn as_array_buffer(&self) -> Option<&ArrayBuffer> {
        self.is_array_buffer()
            .then_some(unsafe { self.ref_array_buffer() })
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use alloc::sync::Arc;

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
            let val = ArrayBuffer::new(ctx.clone(), [-1i8, 0, 22, 5]).unwrap();
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
            let val = ArrayBuffer::new(ctx.clone(), [-1.5f32, 0.0, 2.25]).unwrap();
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

    #[test]
    fn from_raw_parts_external_buffer() {
        use core::sync::atomic::{AtomicBool, Ordering};

        static DROPPED: AtomicBool = AtomicBool::new(false);

        let rt = crate::Runtime::new().unwrap();
        let c = crate::Context::full(&rt).unwrap();
        c.with(|ctx| {
            let buf: alloc::boxed::Box<[u8]> = alloc::vec![1u8, 2, 3, 4].into_boxed_slice();
            let ptr = buf.as_ptr();
            let len = buf.len();
            let ab = unsafe {
                ArrayBuffer::from_raw_parts(
                    ctx.clone(),
                    ptr,
                    len,
                    Box::new(move || {
                        drop(buf);
                        DROPPED.store(true, Ordering::SeqCst);
                    }),
                )
                .unwrap()
            };
            assert_eq!(ab.len(), 4);
            assert_eq!(ab.as_bytes().unwrap(), &[1, 2, 3, 4]);
        });
        rt.run_gc();
        assert!(DROPPED.load(Ordering::SeqCst));
    }

    #[test]
    fn from_raw_parts_error_invokes_drop_fn() {
        use core::sync::atomic::{AtomicBool, Ordering};

        static DROPPED: AtomicBool = AtomicBool::new(false);

        let rt = crate::Runtime::new().unwrap();
        let c = crate::Context::full(&rt).unwrap();
        c.with(|ctx| {
            let buf: alloc::boxed::Box<[u8]> = alloc::vec![1u8, 2, 3, 4].into_boxed_slice();
            let ptr = buf.as_ptr();
            let err = unsafe {
                ArrayBuffer::from_raw_parts(
                    ctx.clone(),
                    ptr,
                    i64::MAX as usize,
                    Box::new(move || {
                        drop(buf);
                        DROPPED.store(true, Ordering::SeqCst);
                    }),
                )
            };
            assert!(err.is_err());
        });
        assert!(DROPPED.load(Ordering::SeqCst));
    }

    #[test]
    fn from_raw_parts_immutable_arc_slices() {
        let buf: Arc<Vec<u8>> = Arc::new((0u8..16).collect());
        let weak = Arc::downgrade(&buf);

        let rt = crate::Runtime::new().unwrap();
        let c = crate::Context::full(&rt).unwrap();
        c.with(|ctx| {
            let mk = |offset: usize, len: usize| -> ArrayBuffer<'_> {
                let clone = buf.clone();
                let ptr = unsafe { clone.as_ptr().add(offset) };
                unsafe {
                    ArrayBuffer::from_raw_parts_immutable(
                        ctx.clone(),
                        ptr,
                        len,
                        Box::new(move || drop(clone)),
                    )
                    .unwrap()
                }
            };
            let full = mk(0, 16);
            let head = mk(0, 4);
            let tail = mk(12, 4);
            let middle = mk(4, 8);

            assert_eq!(full.as_bytes().unwrap(), (0u8..16).collect::<Vec<_>>());
            assert_eq!(head.as_bytes().unwrap(), &[0, 1, 2, 3]);
            assert_eq!(tail.as_bytes().unwrap(), &[12, 13, 14, 15]);
            assert_eq!(middle.as_bytes().unwrap(), (4u8..12).collect::<Vec<_>>());
            assert_eq!(Arc::strong_count(&buf), 5);

            ctx.globals().set("buf", full).unwrap();

            let after: u8 = ctx
                .eval::<u8, _>(
                    r#"
                        const arr = new Uint8Array(buf);
                        arr[0] = 99;
                        arr[0];
                    "#,
                )
                .unwrap();
            assert_eq!(after, 0, "immutable ArrayBuffer must not accept writes");

            let writer = ctx.eval::<(), _>(
                r#"
                    "use strict";
                    new DataView(buf).setUint8(0, 99);
                "#,
            );
            assert!(
                writer.is_err(),
                "DataView write on immutable buffer must throw"
            );
        });

        drop(buf);
        rt.run_gc();
        drop(c);
        drop(rt);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn from_source_vec() {
        let rt = crate::Runtime::new().unwrap();
        let c = crate::Context::full(&rt).unwrap();
        c.with(|ctx| {
            let ab = ArrayBuffer::from_source(ctx.clone(), alloc::vec![1u8, 2, 3, 4]).unwrap();
            assert_eq!(ab.as_bytes().unwrap(), &[1, 2, 3, 4]);
        });
    }

    #[test]
    fn from_source_immutable_arc() {
        let arc: Arc<[u8]> = Arc::from((0u8..8).collect::<Vec<_>>().into_boxed_slice());
        let weak = Arc::downgrade(&arc);
        assert_eq!(Arc::strong_count(&arc), 1);

        let rt = crate::Runtime::new().unwrap();
        let c = crate::Context::full(&rt).unwrap();

        // Case 1: JS drops first while Rust keeps its Arc clone.
        c.with(|ctx| {
            let _ab = ArrayBuffer::from_source_immutable(ctx.clone(), arc.clone()).unwrap();
            assert_eq!(Arc::strong_count(&arc), 2, "Arc cloned into drop closure");
            // _ab drops at end of block; shim runs; closure drops its Arc clone.
        });
        assert_eq!(
            Arc::strong_count(&arc),
            1,
            "drop closure must release its Arc clone when ArrayBuffer is freed"
        );
        assert!(
            weak.upgrade().is_some(),
            "allocation must stay alive while Rust still holds the Arc"
        );

        // Case 2: Rust drops first, JS keeps the buffer (stored in globals).
        c.with(|ctx| {
            let ab = ArrayBuffer::from_source_immutable(ctx.clone(), arc.clone()).unwrap();
            ctx.globals().set("buf", ab).unwrap();
            assert_eq!(Arc::strong_count(&arc), 2);
        });
        drop(arc);
        assert!(
            weak.upgrade().is_some(),
            "allocation must stay alive: JS still holds the buffer via the Arc clone in the closure"
        );
        assert_eq!(
            weak.strong_count(),
            1,
            "only the closure's Arc clone should remain"
        );

        // Drop the context: JS buffer is freed, shim runs, Arc clone released.
        drop(c);
        drop(rt);
        assert!(
            weak.upgrade().is_none(),
            "allocation must be freed after both Rust and JS release their handles"
        );
    }

    #[cfg(feature = "bytes")]
    #[test]
    fn from_source_immutable_bytes() {
        let data: bytes::Bytes = (0u8..8).collect::<Vec<_>>().into();
        let rt = crate::Runtime::new().unwrap();
        let c = crate::Context::full(&rt).unwrap();
        c.with(|ctx| {
            let ab = ArrayBuffer::from_source_immutable(ctx.clone(), data.clone()).unwrap();
            assert_eq!(ab.as_bytes().unwrap(), (0u8..8).collect::<Vec<_>>());
        });
    }
}
