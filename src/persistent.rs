use crate::{qjs, Array, Ctx, FromJs, Function, IntoJs, Object, Result, String, Symbol, Value};
use std::{
    cell::Cell,
    cmp::PartialEq,
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    mem,
};

pub trait Outlive<'t> {
    type Target;
}

macro_rules! outlive_impls {
    ($($type:ident,)*) => {
        $(
            impl<'js, 't> Outlive<'t> for $type<'js> {
                type Target = $type<'t>;
            }
        )*
    };
}

outlive_impls! {
    Value,
    Function,
    Symbol,
    String,
    Object,
    Array,
}

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
/// NOTE: Be careful and ensure that no persistent links outlives the runtime.
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

impl<'t, T> Drop for Persistent<T> {
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

    /// Save the value of an arbitrary type
    pub fn save<'js>(ctx: Ctx<'js>, val: T) -> Persistent<T::Target>
    where
        T: AsRef<Value<'js>> + Outlive<'static>,
    {
        let value = val.as_ref().value;
        mem::forget(val);
        let rt = unsafe { qjs::JS_GetRuntime(ctx.ctx) };

        Persistent::new_raw(rt, value)
    }

    /// Restore the value of an arbitrary type
    pub fn restore<'js>(self, ctx: Ctx<'js>) -> Result<T::Target>
    where
        T: Outlive<'js>,
        T::Target: FromJs<'js>,
    {
        let value = unsafe { Value::from_js_value(ctx, self.value.get()) };
        mem::forget(self);
        T::Target::from_js(ctx, value)
    }

    fn ptr(&self) -> *mut qjs::c_void {
        unsafe { qjs::JS_VALUE_GET_PTR(self.value.get()) }
    }

    fn tag(&self) -> qjs::c_int {
        unsafe { qjs::JS_VALUE_GET_TAG(self.value.get()) }
    }
}

impl<'js, 't, T> FromJs<'js> for Persistent<T>
where
    T: Outlive<'js>,
    T::Target: FromJs<'js> + IntoJs<'js>,
{
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Persistent<T>> {
        let value = T::Target::from_js(ctx, value)?;
        let value = value.into_js(ctx)?;
        let value = value.into_js_value();
        let rt = unsafe { qjs::JS_GetRuntime(ctx.ctx) };

        Ok(Self::new_raw(rt, value))
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
    fn persistent_function() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();

        let func = ctx.with(|ctx| {
            let func: Function = ctx.eval("a => a + 1").unwrap();
            Persistent::save(ctx, func)
        });

        let res: i32 = ctx.with(|ctx| {
            let func = func.clone().restore(ctx).unwrap();
            func.call((2,)).unwrap()
        });
        assert_eq!(res, 3);

        let ctx2 = Context::full(&rt).unwrap();
        let res: i32 = ctx2.with(|ctx| {
            let func = func.restore(ctx).unwrap();
            func.call((0,)).unwrap()
        });
        assert_eq!(res, 1);
    }
}
