use crate::{qjs, Array, Ctx, FromJs, Function, IntoJs, Object, Result, String, Symbol, Value};
use std::{
    cmp::PartialEq,
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
///     ctx.eval::<Persistent<Function>, _>("a => a + 1").unwrap().outlive()
/// });
/// let res: i32 = ctx.with(|ctx| {
///     let func = func.clone().inlive().restore(ctx).unwrap();
///     func.call((2,)).unwrap()
/// });
/// assert_eq!(res, 3);
/// let res: i32 = ctx.with(|ctx| {
///     let func = func.inlive().restore(ctx).unwrap();
///     func.call((0,)).unwrap()
/// });
/// assert_eq!(res, 1);
/// ```
///
/// NOTE: Be careful and ensure that no persistent links outlives the runtime.
///
pub struct Persistent<T> {
    rt: *mut qjs::JSRuntime,
    value: qjs::JSValue,
    marker: PhantomData<T>,
}

impl<T> Clone for Persistent<T> {
    fn clone(&self) -> Self {
        let value = unsafe { qjs::JS_DupValue(self.value) };
        Self {
            rt: self.rt,
            value,
            marker: PhantomData,
        }
    }
}

impl<'t, T> Drop for Persistent<T> {
    fn drop(&mut self) {
        unsafe { qjs::JS_FreeValueRT(self.rt, self.value) };
    }
}

impl<T> Persistent<T> {
    /// Save the value of an arbitrary type
    pub fn save<'js>(ctx: Ctx<'js>, value: T) -> Result<Persistent<T>>
    where
        T: IntoJs<'js>,
    {
        let value = value.into_js(ctx)?;
        let value = value.into_js_value();
        let rt = unsafe { qjs::JS_GetRuntime(ctx.ctx) };

        Ok(Self {
            rt,
            value,
            marker: PhantomData,
        })
    }

    /// Restore the value of an arbitrary type
    pub fn restore<'js>(self, ctx: Ctx<'js>) -> Result<T>
    where
        T: FromJs<'js>,
    {
        let value = unsafe { Value::from_js_value(ctx, self.value) }?;
        mem::forget(self);
        T::from_js(ctx, value)
    }

    pub fn outlive(self) -> Persistent<T::Target>
    where
        T: Outlive<'static>,
    {
        let Self { rt, value, .. } = self;
        mem::forget(self);
        Persistent {
            rt,
            value,
            marker: PhantomData,
        }
    }

    pub fn inlive<'js>(self) -> Persistent<T::Target>
    where
        T: Outlive<'js>,
    {
        let Self { rt, value, .. } = self;
        mem::forget(self);
        Persistent {
            rt,
            value,
            marker: PhantomData,
        }
    }
}

impl<'js, T> FromJs<'js> for Persistent<T>
where
    T: FromJs<'js> + IntoJs<'js>,
{
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Persistent<T>> {
        let value = T::from_js(ctx, value)?;
        let rt = unsafe { qjs::JS_GetRuntime(ctx.ctx) };
        let value = value.into_js(ctx)?.into_js_value();

        Ok(Self {
            rt,
            value,
            marker: PhantomData,
        })
    }
}

impl<'js, 't, T> IntoJs<'js> for Persistent<T>
where
    T: Outlive<'t>,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let value = unsafe { Value::from_js_value(ctx, self.value) }?;
        mem::forget(self);
        value.into_js(ctx)
    }
}

unsafe impl<T> Send for Persistent<T> {}
unsafe impl<T> Sync for Persistent<T> {}

impl<T> Hash for Persistent<T> {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        unsafe { qjs::JS_VALUE_GET_PTR(self.value) }.hash(state);
        unsafe { qjs::JS_VALUE_GET_NORM_TAG(self.value) }.hash(state);
    }
}

impl<T, S> PartialEq<Persistent<S>> for Persistent<T> {
    fn eq(&self, other: &Persistent<S>) -> bool {
        (unsafe { qjs::JS_VALUE_GET_NORM_TAG(self.value) }
            == unsafe { qjs::JS_VALUE_GET_NORM_TAG(other.value) })
            && (unsafe { qjs::JS_VALUE_GET_PTR(self.value) }
                == unsafe { qjs::JS_VALUE_GET_PTR(other.value) })
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
            ctx.eval::<Persistent<Function>, _>("a => a + 1")
                .unwrap()
                .outlive()
        });

        let res: i32 = ctx.with(|ctx| {
            let func = func.clone().inlive().restore(ctx).unwrap();
            func.call((2,)).unwrap()
        });
        assert_eq!(res, 3);

        let ctx2 = Context::full(&rt).unwrap();
        let res: i32 = ctx2.with(|ctx| {
            let func = func.inlive().restore(ctx).unwrap();
            func.call((0,)).unwrap()
        });
        assert_eq!(res, 1);
    }
}
