use crate::{
    qjs, Array, BigInt, Ctx, Error, FromJs, IntoJs, Object, Result, String, Symbol, Value,
};
use std::{
    cell::Cell,
    cmp::PartialEq,
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    mem,
};

/// The trait to help break lifetime rules when JS objects leaves current context via [`Persistent`] wrapper.
pub unsafe trait Outlive<'t> {
    /// The target which has the same type as a `Self` but with another lifetime `'t`
    type Target<'to>;
}

macro_rules! outlive_impls {
    ($($type:ident,)*) => {
        $(
            unsafe impl<'js> Outlive<'js> for $type<'js> {
                type Target<'to> = $type<'to>;
            }
        )*
    };
}

outlive_impls! {
    Value,
    Symbol,
    String,
    Object,
    Array,
    BigInt,
}

macro_rules! impl_outlive{
    ($($($ty:ident)::*$([$($g:ident),+])*),*$(,)?) => {
        $(
            unsafe impl<'js,$($($g,)*)*> Outlive<'js> for $($ty)::*$(<$($g,)*>)*
            where Self: 'static,
                  $($($g: 'static,)*)*
            {
                type Target<'to> = $($ty)::*$(<$($g,)*>)*;
            }
        )*
    };
}

impl_outlive!(
    u8,
    u16,
    u32,
    u64,
    usize,
    u128,
    i8,
    i16,
    i32,
    i64,
    isize,
    i128,
    std::string::String,
    Vec[T]
);

/// The wrapper for JS values to keep it from GC
///
/// For example you can store JS functions for later use.
/// ```
/// # use rquickjs::{Runtime, Context, Persistent, Function};
/// # let rt = Runtime::new().unwrap();
/// # let ctx = Context::full(&rt).unwrap();
/// let func = ctx.with(|ctx| {
///     Persistent::save(ctx, ctx.eval::<Function, _>("a => a + 1").unwrap())
/// });
/// let res: i32 = ctx.with(|ctx| {
///     let func = func.clone().restore(ctx).unwrap();
///     func.call((2,)).unwrap()
/// });
/// assert_eq!(res, 3);
/// let res: i32 = ctx.with(|ctx| {
///     let func = func.restore(ctx).unwrap();
///     func.call((0,)).unwrap()
/// });
/// assert_eq!(res, 1);
/// ```
///
/// It is an error (`Error::UnrelatedRuntime`) to restore the `Persistent` in a
/// context who isn't part of the original `Runtime`.
///
/// NOTE: Be careful and ensure that no persistent links outlives the runtime,
/// otherwise Runtime will abort the process when dropped.
///
pub struct Persistent<T> {
    pub(crate) rt: *mut qjs::JSRuntime,
    pub(crate) value: Cell<qjs::JSValue>,
    marker: PhantomData<T>,
}

impl<T> Clone for Persistent<T> {
    fn clone(&self) -> Self {
        let value = unsafe { qjs::JS_DupValue(self.value.get()) };
        Self::new_raw(self.rt, value)
    }
}

impl<T> fmt::Debug for Persistent<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Persistent")
            .field("rt", &self.rt)
            .field("ptr", &unsafe { qjs::JS_VALUE_GET_PTR(self.value.get()) })
            .finish()
    }
}

impl<T> Drop for Persistent<T> {
    fn drop(&mut self) {
        unsafe { qjs::JS_FreeValueRT(self.rt, self.value.get()) };
    }
}

impl<T> Persistent<T> {
    fn new_raw(rt: *mut qjs::JSRuntime, value: qjs::JSValue) -> Self {
        Self {
            rt,
            value: Cell::new(value),
            marker: PhantomData,
        }
    }

    #[cfg(feature = "classes")]
    pub(crate) fn mark_raw(&self, mark_func: qjs::JS_MarkFunc) {
        let value = self.value.get();
        if unsafe { qjs::JS_VALUE_HAS_REF_COUNT(value) } {
            unsafe { qjs::JS_MarkValue(self.rt, value, mark_func) };
            if 0 == unsafe { qjs::JS_ValueRefCount(value) } {
                self.value.set(qjs::JS_UNDEFINED);
            }
        }
    }

    /// Save the value of an arbitrary type
    pub fn save<'js>(ctx: Ctx<'js>, val: T) -> Persistent<T::Target<'static>>
    where
        T: Outlive<'js>,
    {
        todo!();

        //Persistent::new_raw(rt, value)
    }

    /// Restore the value of an arbitrary type
    pub fn restore<'js>(self, ctx: Ctx<'js>) -> Result<T::Target<'js>>
    where
        T: Outlive<'static>,
        T::Target<'js>: FromJs<'js>,
    {
        let ctx_runtime_ptr = unsafe { qjs::JS_GetRuntime(ctx.as_ptr()) };
        if self.rt != ctx_runtime_ptr {
            return Err(Error::UnrelatedRuntime);
        }
        let value = unsafe { Value::from_js_value(ctx, self.value.get()) };
        mem::forget(self);
        T::Target::<'js>::from_js(ctx, value)
    }

    fn ptr(&self) -> *mut qjs::c_void {
        unsafe { qjs::JS_VALUE_GET_PTR(self.value.get()) }
    }

    fn tag(&self) -> qjs::c_int {
        unsafe { qjs::JS_VALUE_GET_TAG(self.value.get()) }
    }
}

impl<'js, T, R> FromJs<'js> for Persistent<R>
where
    R: Outlive<'static, Target<'js> = T>,
    T: Outlive<'js, Target<'static> = R> + FromJs<'js>,
{
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Persistent<R>> {
        let value = T::from_js(ctx, value)?;
        Ok(Persistent::save(ctx, value))
    }
}

impl<'js, 't, T> IntoJs<'js> for Persistent<T>
where
    T: Outlive<'t>,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let value = unsafe { Value::from_js_value(ctx, self.value.get()) };
        mem::forget(self);
        value.into_js(ctx)
    }
}

#[cfg(feature = "parallel")]
unsafe impl<T> Send for Persistent<T> {}

impl<T> Hash for Persistent<T> {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.ptr().hash(state);
        self.tag().hash(state);
    }
}

impl<T, S> PartialEq<Persistent<S>> for Persistent<T> {
    fn eq(&self, other: &Persistent<S>) -> bool {
        (self.tag() == other.tag()) && (self.ptr() == other.ptr())
    }
}

impl<T> Eq for Persistent<T> {}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    #[should_panic(expected = "UnrelatedRuntime")]
    fn different_runtime() {
        let rt1 = Runtime::new().unwrap();
        let ctx = Context::full(&rt1).unwrap();

        let persistent_v = ctx.with(|ctx| {
            let v: Value = ctx.eval("1").unwrap();
            Persistent::save(ctx, v)
        });

        let rt2 = Runtime::new().unwrap();
        let ctx = Context::full(&rt2).unwrap();
        ctx.with(|ctx| {
            let _ = persistent_v.clone().restore(ctx).unwrap();
        });
    }

    #[test]
    fn persistent_function() {
        todo!()
    }

    #[test]
    fn persistent_value() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();

        let persistent_v = ctx.with(|ctx| {
            let v: Value = ctx.eval("1").unwrap();
            Persistent::save(ctx, v)
        });

        ctx.with(|ctx| {
            let v = persistent_v.clone().restore(ctx).unwrap();
            ctx.globals().set("v", v).unwrap();
            let eq: Value = ctx.eval("v == 1").unwrap();
            assert!(eq.as_bool().unwrap());
        });
    }
}
