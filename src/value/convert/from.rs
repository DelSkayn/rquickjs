use super::FromJs;
use crate::{value, Array, Ctx, Error, FromAtom, Function, Object, Result, String, Value};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, LinkedList, VecDeque},
    convert::TryFrom,
    hash::Hash,
    string::String as StdString,
};

impl<'js> FromJs<'js> for Value<'js> {
    fn from_js(_: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Ok(value)
    }
}

impl<'js> FromJs<'js> for StdString {
    fn from_js(_ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        match value {
            Value::String(x) => Ok(x.to_string()?),
            x => Err(Error::FromJs {
                from: x.type_name(),
                to: "string",
                message: None,
            }),
        }
    }
}

macro_rules! fromjs_impls {
    // for list-like Rust types
    (list: $($type:ident $(($($guard:tt)*))* ,)*) => {
        $(
            impl<'js, T> FromJs<'js> for $type<T>
            where
                T: FromJs<'js> $(+ $($guard)*)*,
            {
                fn from_js(_ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    let array = match value {
                        Value::Array(array) => array,
                        other => {
                            return Err(Error::FromJs{
                                from: other.type_name(),
                                to: "array",
                                message: None,
                            });
                        }
                    };
                    array.iter().collect::<Result<$type<_>>>()
                }
            }
        )*
    };

    // for map-like Rust types
    (map: $($type:ident $(($($guard:tt)*))* ,)*) => {
        $(
            impl<'js, K, V> FromJs<'js> for $type<K, V>
            where
                K: FromAtom<'js> $(+ $($guard)*)*,
                V: FromJs<'js>,
            {
                fn from_js(_ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    let object = match value {
                        Value::Object(object) => object,
                        Value::Array(array) => array.into_object(),
                        Value::Function(func) => func.into_object(),
                        other => {
                            return Err(Error::FromJs{
                                from: other.type_name(),
                                to: "object",
                                message: None,
                            });
                        }
                    };
                    object.own_props(true)?.collect::<Result<$type<_, _>>>()
                }
            }
        )*
    };

    // for primitive types which needs coercion
    ($($type:ty => $coerce:ident,)*) => {
        $(
            impl<'js> FromJs<'js> for $type {
                fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    let type_name = value.type_name();
                    ctx.$coerce(value).map_err(|error| {
                        if error.is_exception() {
                            Error::FromJs{
                                from: type_name,
                                to: stringify!($type),
                                message: Some(error.to_string()),
                            }
                        } else {
                            error
                        }
                    })
                }
            }
        )*
    };

    // for primitive types which needs coercion and optional try_from
    ($($type:ty: $origtype:ty,)*) => {
        $(
            impl<'js> FromJs<'js> for $type {
                fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    <$origtype>::from_js(ctx, value).and_then(|value| {
                        <$type>::try_from(value).map_err(|_error| {
                            Error::FromJs{
                                from: stringify!($origtype),
                                to: stringify!($type),
                                message: None,
                            }
                        })
                    })
                }
            }
        )*
    };

    // for JS Value types
    ($($type:ident $(($($stype:ident),*))*: $typename:literal,)*) => {
        $(
            impl<'js> FromJs<'js> for $type<'js> {
                fn from_js(_: Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    match value {
                        Value::$type(value) => Ok(value),
                        $($(
                        Value::$stype(value) => Ok(value.into_object()),
                        )*)*
                        other => Err(Error::FromJs{
                            from: other.type_name(),
                            to: $typename,
                            message: None,
                        }),
                    }
                }
            }
        )*
    };
}

fromjs_impls! {
    i8: i32,
    u8: i32,

    i16: i32,
    u16: i32,

    u32: u64,
}

fromjs_impls! {
    bool => coerce_bool,

    i32 => coerce_i32,

    i64 => coerce_i64,
    u64 => coerce_u64,

    f64 => coerce_f64,
}

fromjs_impls! {
    String: "string",
    Function: "function",
    Array: "array",
    Object(Array, Function): "object",
}

fromjs_impls! {
    list:
    Vec,
    VecDeque,
    LinkedList,
    HashSet (Eq + Hash),
    BTreeSet (Eq + Ord),
}

fromjs_impls! {
    map:
    HashMap (Eq + Hash),
    BTreeMap (Eq + Ord),
}

impl<'js> FromJs<'js> for () {
    fn from_js(_: Ctx<'js>, _: Value<'js>) -> Result<Self> {
        Ok(())
    }
}

impl<'js, T> FromJs<'js> for Option<T>
where
    T: FromJs<'js>,
{
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        match value {
            Value::Undefined | Value::Uninitialized | Value::Null => Ok(None),
            other => T::from_js(ctx, other).map(Some),
        }
    }
}

impl<'js, T> FromJs<'js> for Result<T>
where
    T: FromJs<'js>,
{
    //TODO this function seems a bit hacky.
    //Expections are generally handled when returned from a function
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        unsafe {
            match value::handle_exception(ctx, value.into_js_value()) {
                Ok(x) => T::from_js(ctx, Value::from_js_value(ctx, x)?).map(Ok),
                Err(e) => Ok(Err(e)),
            }
        }
    }
}
