use super::ToJs;
use crate::{Array, Ctx, Function, IteratorJs, Object, Result, String, ToAtom, Value};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, LinkedList, VecDeque},
    result::Result as StdResult,
    string::String as StdString,
};

impl<'js> ToJs<'js> for Value<'js> {
    fn to_js(self, _: Ctx<'js>) -> Result<Value<'js>> {
        Ok(self)
    }
}

impl<'js> ToJs<'js> for StdString {
    fn to_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let s = String::from_str(ctx, self.as_str())?;
        Ok(Value::String(s))
    }
}

impl<'js, 'a> ToJs<'js> for &'a str {
    fn to_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let s = String::from_str(ctx, self)?;
        Ok(Value::String(s))
    }
}

impl<'js> ToJs<'js> for () {
    fn to_js(self, _: Ctx<'js>) -> Result<Value<'js>> {
        Ok(Value::Undefined)
    }
}

impl<'js, T> ToJs<'js> for Option<T>
where
    T: ToJs<'js>,
{
    fn to_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        Ok(match self {
            Some(value) => value.to_js(ctx)?,
            _ => Value::Undefined,
        })
    }
}

impl<'js, T, E> ToJs<'js> for StdResult<T, E>
where
    T: ToJs<'js>,
    E: ToJs<'js>,
{
    fn to_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        Ok(match self {
            Ok(value) => value.to_js(ctx)?,
            Err(error) => error.to_js(ctx)?,
        })
    }
}

macro_rules! tojs_impls {
    // for JS Value sub-types
    ($($type:ident,)*) => {
        $(
            impl<'js> ToJs<'js> for $type<'js> {
                fn to_js(self, _: Ctx<'js>) -> Result<Value<'js>> {
                    Ok(Value::$type(self))
                }
            }
        )*
    };

    // for list-like Rust types
    (list: $($type:ident,)*) => {
        $(
            impl<'js, T> ToJs<'js> for $type<T>
            where
                T: ToJs<'js>,
            {
                fn to_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
                    self.into_iter()
                        .collect_js::<Array>(ctx)
                        .map(Value::Array)
                }
            }
        )*
    };

    // for map-like Rust types
    (map: $($type:ident,)*) => {
        $(
            impl<'js, K, V> ToJs<'js> for $type<K, V>
            where
                K: ToAtom<'js>,
                V: ToJs<'js>,
            {
                fn to_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
                    self.into_iter()
                        .collect_js::<Object>(ctx)
                        .map(Value::Object)
                }
            }
        )*
    };

    // for primitive types
    ($($type:ty: $jstype:ident,)*) => {
        $(
            impl<'js> ToJs<'js> for $type {
                fn to_js(self, _: Ctx<'js>) -> Result<Value<'js>> {
                    Ok(Value::$jstype(self as _))
                }
            }
        )*
    };

    // for primitive types which needs error-prone casting (ex. u32 -> i32)
    ($($type:ty => $totype:ty: $jstype:ident,)*) => {
        $(
            impl<'js> ToJs<'js> for $type {
                fn to_js(self, _: Ctx<'js>) -> Result<Value<'js>> {
                    use std::convert::TryFrom;
                    let val = <$totype>::try_from(self).map_err(|_| {
                        $crate::Error::ToJsConversion {
                            from: stringify!($type),
                            to: stringify!($totype),
                            message: None,
                        }
                    })?;
                    Ok(Value::$jstype(val as _))
                }
            }
        )*
    };
}

tojs_impls! {
    String,
    Array,
    Object,
    Function,
}

tojs_impls! {
    list:
    Vec,
    VecDeque,
    LinkedList,
    HashSet,
    BTreeSet,
}

tojs_impls! {
    map:
    HashMap,
    BTreeMap,
}

tojs_impls! {
    bool: Bool,

    i8: Int,
    i16: Int,
    i32: Int,

    u8: Int,
    u16: Int,

    f32: Float,
    f64: Float,
}

tojs_impls! {
    i64 => i32: Int,
    u32 => i32: Int,
    u64 => i32: Int,
}
