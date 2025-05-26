use crate::{Ctx, FromJs, Result, StdString, String, Value, convert::Coerced, qjs};
use std::{
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

impl<T> AsRef<T> for Coerced<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> AsMut<T> for Coerced<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> Deref for Coerced<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Coerced<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Coerce a value to a string in the same way JavaScript would coerce values.
impl<'js> FromJs<'js> for Coerced<String<'js>> {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Ok(Coerced(unsafe {
            let result = qjs::JS_ToString(ctx.as_ptr(), value.as_js_value());
            ctx.handle_exception(result)?;
            // result should be a string now
            // String itself will check for the tag when debug_assertions are enabled
            // but is should always be string
            String::from_js_value(ctx.clone(), result)
        }))
    }
}

/// Coerce a value to a string in the same way JavaScript would coerce values.
impl<'js> FromJs<'js> for Coerced<StdString> {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        <Coerced<String>>::from_js(ctx, value)
            .and_then(|string| string.to_string())
            .map(Coerced)
    }
}

macro_rules! coerce_impls {
	  ($($(#[$meta:meta])* $type:ident $func:ident,)*) => {
		    $(
            $(#[$meta])*
            impl<'js> FromJs<'js> for Coerced<$type> {
                fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    let mut result = MaybeUninit::uninit();
                    Ok(Coerced(unsafe {
                        if 0 > qjs::$func(ctx.as_ptr(), result.as_mut_ptr(), value.as_js_value()) {
                            return Err(ctx.raise_exception());
                        }
                        result.assume_init()
                    }))
                }
            }
        )*
	  };
}

coerce_impls! {
    /// Coerce a value to a `i32` in the same way JavaScript would coerce values
    i32 JS_ToInt32,
    /// Coerce a value to a `i64` in the same way JavaScript would coerce values
    i64 JS_ToInt64Ext,
    /// Coerce a value to a `u64` in the same way JavaScript would coerce values
    u64 JS_ToIndex,
    /// Coerce a value to a `f64` in the same way JavaScript would coerce values
    f64 JS_ToFloat64,
}

/// Coerce a value to a `bool` in the same way JavaScript would coerce values
impl<'js> FromJs<'js> for Coerced<bool> {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Ok(Coerced(unsafe {
            let res = qjs::JS_ToBool(ctx.as_ptr(), value.as_js_value());
            if 0 > res {
                return Err(ctx.raise_exception());
            }
            res == 1
        }))
    }
}
