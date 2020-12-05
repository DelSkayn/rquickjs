use super::FromJs;
use crate::{value, Array, Ctx, Error, FromAtom, Function, Object, Result, String, Symbol, Value};
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

fn not_enough_values() -> Error {
    Error::FromJs {
        from: "array",
        to: "tuple",
        message: Some("Not enough values".into()),
    }
}

fn too_many_values() -> Error {
    Error::FromJs {
        from: "array",
        to: "tuple",
        message: Some("Too many values".into()),
    }
}

macro_rules! from_js_impls {
    // for tuple types
    (tup: $($($type:ident)*,)*) => {
        $(
            impl<'js, $($type,)*> FromJs<'js> for ($($type,)*)
            where
                $($type: FromJs<'js>,)*
            {
                #[allow(non_snake_case)]
                fn from_js(_ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    let array = match value {
                        Value::Array(array) => array,
                        other => {
                            return Err(Error::FromJs{
                                from: other.type_name(),
                                to: "tuple",
                                message: None,
                            });
                        }
                    };

                    let tuple_len = 0 $(+ from_js_impls!(@one $type))*;
                    let array_len = array.len();
                    if array_len != tuple_len {
                        return if array_len < tuple_len {
                            Err(not_enough_values())
                        } else {
                            Err(too_many_values())
                        };
                    }

                    Ok((
                        $(array.get::<$type>(from_js_impls!(@idx $type))?,)*
                    ))
                }
            }
        )*
    };

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
                    object.own_props(true).collect::<Result<$type<_, _>>>()
                }
            }
        )*
    };

    // for primitive types which needs coercion
    (val: $($type:ty => $coerce:ident,)*) => {
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
    (val: $($type:ty: $origtype:ty,)*) => {
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
    (js: $($type:ident $(($($stype:ident),*))*: $typename:literal,)*) => {
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

    (@one $($t:tt)*) => { 1 };

    (@idx A) => { 0 };
    (@idx B) => { 1 };
    (@idx C) => { 2 };
    (@idx D) => { 3 };
    (@idx E) => { 4 };
    (@idx F) => { 5 };
    (@idx G) => { 6 };
    (@idx H) => { 7 };
    (@idx I) => { 8 };
    (@idx J) => { 9 };
    (@idx K) => { 10 };
    (@idx L) => { 11 };
    (@idx M) => { 12 };
    (@idx N) => { 13 };
    (@idx O) => { 14 };
    (@idx P) => { 15 };
}

from_js_impls! {
    val:
    i8: i32,
    u8: i32,

    i16: i32,
    u16: i32,

    u32: u64,
}

from_js_impls! {
    val:
    bool => coerce_bool,

    i32 => coerce_i32,

    i64 => coerce_i64,
    u64 => coerce_u64,

    f64 => coerce_f64,
}

from_js_impls! {
    js:
    String: "string",
    Symbol: "symbol",
    Function: "function",
    Array: "array",
    Object(Array, Function): "object",
}

from_js_impls! {
    tup:
    A,
    A B,
    A B C,
    A B C D,
    A B C D E,
    A B C D E F,
    A B C D E F G,
    A B C D E F G H,
    A B C D E F G H I,
    A B C D E F G H I J,
    A B C D E F G H I J K,
    A B C D E F G H I J K L,
    A B C D E F G H I J K L M,
    A B C D E F G H I J K L M N,
    A B C D E F G H I J K L M N O,
    A B C D E F G H I J K L M N O P,
}

from_js_impls! {
    list:
    Vec,
    VecDeque,
    LinkedList,
    HashSet (Eq + Hash),
    BTreeSet (Eq + Ord),
}

from_js_impls! {
    map:
    HashMap (Eq + Hash),
    BTreeMap (Eq + Ord),
}

impl<'js> FromJs<'js> for f32 {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        f64::from_js(ctx, value).map(|value| value as _)
    }
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

macro_rules! into_fns {
    ($($(#[$metas:meta])* $name:ident: $type:literal => $variant:ident,)*) => {
        $(
            $(#[$metas])*
            pub fn $name(self) -> Result<$variant<'js>> {
                if let Value::$variant(value) = self {
                    Ok(value)
                } else {
                    Err(Error::FromJs { from: self.type_name(), to: $type, message: None })
                }
            }
        )*
    };
}

impl<'js> Value<'js> {
    into_fns! {
        /// Try convert into object
        into_object: "object" => Object,
        /// Try convert into array
        into_array: "array" => Array,
        /// Try convert into function
        into_function: "function" => Function,
        /// Try convert into string
        into_string: "string" => String,
        /// Try convert into symbol
        into_symbol: "symbol" => Symbol,
    }
}
