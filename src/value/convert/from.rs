use crate::{
    handle_exception, Array, Ctx, Error, FromAtom, FromJs, Object, Result, StdString, String, Type,
    Value,
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, LinkedList, VecDeque},
    hash::Hash,
};

impl<'js> FromJs<'js> for Value<'js> {
    fn from_js(_: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Ok(value)
    }
}

impl<'js> FromJs<'js> for StdString {
    fn from_js(_ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        String::from_value(value).and_then(|string| string.to_string())
    }
}

fn tuple_match_size(actual: usize, expected: usize) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(Error::new_from_js_message(
            "array",
            "tuple",
            if actual < expected {
                "Not enough values"
            } else {
                "Too many values"
            },
        ))
    }
}

fn number_match_range<T: PartialOrd>(
    val: T,
    min: T,
    max: T,
    from: &'static str,
    to: &'static str,
) -> Result<()> {
    if val < min {
        Err(Error::new_from_js_message(from, to, "Underflow"))
    } else if val > max {
        Err(Error::new_from_js_message(from, to, "Overflow"))
    } else {
        Ok(())
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
                fn from_js(_ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    let array = Array::from_value(value)?;

                    let tuple_len = 0 $(+ from_js_impls!(@one $type))*;
                    let array_len = array.len();
                    tuple_match_size(array_len, tuple_len)?;

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
                    let array = Array::from_value(value)?;
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
                    let object = Object::from_value(value)?;
                    object.own_props(true).collect::<Result<$type<_, _>>>()
                }
            }
        )*
    };

    // for basic primitive types (int and float)
    // (ex. f64 => Float as_float Int as_int)
    (val: $($type:ty => $($jstype:ident $getfn:ident)*,)*) => {
        $(
            impl<'js> FromJs<'js> for $type {
                fn from_js(_ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    let type_ = value.type_of();
                    match type_ {
                        $(Type::$jstype => Ok(unsafe { value.$getfn() } as _),)*
                        _ => Err(Error::new_from_js(type_.as_str(), stringify!($type))),
                    }
                }
            }
        )*
    };

    // for other primitive types
    (val: $($base:ident: $($type:ident)*,)*) => {
        $(
            $(
                impl<'js> FromJs<'js> for $type {
                    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
                        let num = <$base>::from_js(ctx, value)?;
                        number_match_range(num, $type::MIN as $base, $type::MAX as $base, stringify!($base), stringify!($type))?;
                        Ok(num as $type)
                    }
                }
            )*
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
    i32: i8 u8 i16 u16,
    f64: u32 u64 i64,
}

from_js_impls! {
    val:
    bool => Bool get_bool,
    i32 => Int get_int,
    f64 => Float get_float Int get_int,
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

/// Convert from JS as any
impl<'js> FromJs<'js> for () {
    fn from_js(_: Ctx<'js>, _: Value<'js>) -> Result<Self> {
        Ok(())
    }
}

/// Convert from JS as optional
impl<'js, T> FromJs<'js> for Option<T>
where
    T: FromJs<'js>,
{
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        if value.type_of().is_void() {
            Ok(None)
        } else {
            T::from_js(ctx, value).map(Some)
        }
    }
}

/// Convert from JS as result
impl<'js, T> FromJs<'js> for Result<T>
where
    T: FromJs<'js>,
{
    //TODO this function seems a bit hacky.
    //Expections are generally handled when returned from a function
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        unsafe {
            match handle_exception(ctx, value.into_js_value()) {
                Ok(val) => T::from_js(ctx, Value::from_js_value(ctx, val)).map(Ok),
                Err(error) => Ok(Err(error)),
            }
        }
    }
}
