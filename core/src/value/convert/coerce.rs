use crate::{
    get_exception, handle_exception, qjs, Coerced, Ctx, Error, FromJs, Result, StdString, String,
    Value,
};
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

/// Coerce a value to a string in the same way javascript would coerce values.
impl<'js> FromJs<'js> for Coerced<String<'js>> {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Ok(Coerced(unsafe {
            let result = qjs::JS_ToString(ctx.ctx, value.as_js_value());
            handle_exception(ctx, result)?;
            // result should be a string now
            // String itself will check for the tag when debug_assertions are enabled
            // but is should always be string
            String::from_js_value(ctx, result)
        }))
    }
}

/// Coerce a value to a string in the same way javascript would coerce values.
impl<'js> FromJs<'js> for Coerced<StdString> {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
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
                fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    let mut result = MaybeUninit::uninit();
                    Ok(Coerced(unsafe {
                        if 0 > qjs::$func(ctx.ctx, result.as_mut_ptr(), value.as_js_value()) {
                            let type_ = value.type_of();
                            let error = get_exception(ctx);
                            return Err(Error::new_from_js_message(type_.as_str(), stringify!($type), error.to_string()));
                        }
                        result.assume_init()
                    }))
                }
            }
        )*
	  };
}

coerce_impls! {
    /// Coerce a value to a `i32` in the same way javascript would coerce values
    i32 JS_ToInt32,
    /// Coerce a value to a `i64` in the same way javascript would coerce values
    i64 JS_ToInt64Ext,
    /// Coerce a value to a `u64` in the same way javascript would coerce values
    u64 JS_ToIndex,
    /// Coerce a value to a `f64` in the same way javascript would coerce values
    f64 JS_ToFloat64,
}

/// Coerce a value to a `bool` in the same way javascript would coerce values
impl<'js> FromJs<'js> for Coerced<bool> {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Ok(Coerced(unsafe {
            let res = qjs::JS_ToBool(ctx.ctx, value.as_js_value());
            if 0 > res {
                let type_ = value.type_of();
                let error = get_exception(ctx);
                return Err(Error::new_from_js_message(
                    type_.as_str(),
                    stringify!($type),
                    error.to_string(),
                ));
            }
            res == 1
        }))
    }
}
