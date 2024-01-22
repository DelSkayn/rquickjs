use crate::{
    convert::List, Array, Ctx, Error, FromAtom, FromJs, Object, Result, StdString, String, Type,
    Value,
};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, LinkedList, VecDeque},
    hash::{BuildHasher, Hash},
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
    time::{Duration, SystemTime},
};

#[cfg(feature = "either")]
use either::{Either, Left, Right};

#[cfg(feature = "indexmap")]
use indexmap::{IndexMap, IndexSet};

impl<'js> FromJs<'js> for Value<'js> {
    fn from_js(_: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Ok(value)
    }
}

impl<'js> FromJs<'js> for StdString {
    fn from_js(_ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        String::from_value(value).and_then(|string| string.to_string())
    }
}

/// Convert from JS as any
impl<'js> FromJs<'js> for () {
    fn from_js(_: &Ctx<'js>, _: Value<'js>) -> Result<Self> {
        Ok(())
    }
}

/// Convert from JS as optional
impl<'js, T> FromJs<'js> for Option<T>
where
    T: FromJs<'js>,
{
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
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
    // TODO this function seems a bit hacky.
    // Exceptions are generally by the marshalling handled when returned callback.
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        unsafe {
            match ctx.handle_exception(value.into_js_value()) {
                Ok(val) => T::from_js(ctx, Value::from_js_value(ctx.clone(), val)).map(Ok),
                Err(error) => Ok(Err(error)),
            }
        }
    }
}

/// Convert from JS to either
#[cfg(feature = "either")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "either")))]
impl<'js, L, R> FromJs<'js> for Either<L, R>
where
    L: FromJs<'js>,
    R: FromJs<'js>,
{
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        L::from_js(ctx, value.clone()).map(Left).or_else(|error| {
            if error.is_from_js() {
                R::from_js(ctx, value).map(Right)
            } else {
                Err(error)
            }
        })
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
    // for reference types
    (ref: $($(#[$meta:meta])* $type:ident,)*) => {
        $(
            $(#[$meta])*
            impl<'js, T> FromJs<'js> for $type<T>
            where
                T: FromJs<'js>,
            {
                fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    T::from_js(ctx, value).map($type::new)
                }
            }
        )*
    };

    // for tuple types
    (tup: $($($type:ident)*,)*) => {
        $(
            impl<'js, $($type,)*> FromJs<'js> for List<($($type,)*)>
            where
                $($type: FromJs<'js>,)*
            {
                fn from_js(_ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    let array = Array::from_value(value)?;

                    let tuple_len = 0 $(+ from_js_impls!(@one $type))*;
                    let array_len = array.len();
                    tuple_match_size(array_len, tuple_len)?;

                    Ok(List((
                        $(array.get::<$type>(from_js_impls!(@idx $type))?,)*
                    )))
                }
            }
        )*
    };

    // for list-like Rust types
    (list: $($(#[$meta:meta])* $type:ident $({$param:ident: $($pguard:tt)*})* $(($($guard:tt)*))*,)*) => {
        $(
            $(#[$meta])*
            impl<'js, T $(,$param)*> FromJs<'js> for $type<T $(,$param)*>
            where
                T: FromJs<'js> $(+ $($guard)*)*,
                $($param: $($pguard)*,)*
            {
                fn from_js(_ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    let array = Array::from_value(value)?;
                    array.iter().collect::<Result<_>>()
                }
            }
        )*
    };

    // for map-like Rust types
    (map: $($(#[$meta:meta])* $type:ident $({$param:ident: $($pguard:tt)*})* $(($($guard:tt)*))*,)*) => {
        $(
            $(#[$meta])*
            impl<'js, K, V $(,$param)*> FromJs<'js> for $type<K, V $(,$param)*>
            where
                K: FromAtom<'js> $(+ $($guard)*)*,
                V: FromJs<'js>,
                $($param: $($pguard)*,)*
            {
                fn from_js(_ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
                    let object = Object::from_value(value)?;
                    object.props().collect::<Result<_>>()
                }
            }
        )*
    };

    // for basic primitive types (int and float)
    // (ex. f64 => Float as_float Int as_int)
    (val: $($type:ty => $($jstype:ident $getfn:ident)*,)*) => {
        $(
            impl<'js> FromJs<'js> for $type {
                fn from_js(_ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
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
                    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
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
    f64: u32 u64 i64 usize isize,
}

from_js_impls! {
    val:
    bool => Bool get_bool,
    i32 => Int get_int,
    f64 => Float get_float Int get_int,
}

from_js_impls! {
    ref:
    Box,
    Rc,
    Arc,
    Cell,
    RefCell,
    Mutex,
    RwLock,
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
    /// Convert from JS array to Rust vector
    Vec,
    /// Convert from JS array to Rust vector deque
    VecDeque,
    /// Convert from JS array to Rust linked list
    LinkedList,
    /// Convert from JS array to Rust hash set
    HashSet {S: Default + BuildHasher} (Eq + Hash),
    /// Convert from JS array to Rust btree set
    BTreeSet (Eq + Ord),
    /// Convert from JS array to Rust index set
    #[cfg(feature = "indexmap")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "indexmap")))]
    IndexSet {S: Default + BuildHasher} (Eq + Hash),
}

from_js_impls! {
    map:
    /// Convert from JS object to Rust hash map
    HashMap {S: Default + BuildHasher} (Eq + Hash),
    /// Convert from JS object to Rust btree map
    BTreeMap (Eq + Ord),
    /// Convert from JS object to Rust index map
    #[cfg(feature = "indexmap")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "indexmap")))]
    IndexMap {S: Default + BuildHasher} (Eq + Hash),
}

impl<'js> FromJs<'js> for f32 {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        f64::from_js(ctx, value).map(|value| value as _)
    }
}

fn date_to_millis<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> Result<i64> {
    let global = ctx.globals();
    let date_ctor: Object = global.get("Date")?;

    let value = Object::from_value(value)?;

    if !value.is_instance_of(&date_ctor) {
        return Err(Error::new_from_js("Object", "Date"));
    }

    let get_time_fn: crate::Function = value.get("getTime")?;

    get_time_fn.call((crate::function::This(value),))
}

impl<'js> FromJs<'js> for SystemTime {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<SystemTime> {
        let millis = date_to_millis(ctx, value)?;

        if millis >= 0 {
            // since unix epoch
            SystemTime::UNIX_EPOCH
                .checked_add(Duration::from_millis(millis as _))
                .ok_or_else(|| {
                    Error::new_from_js_message("Date", "SystemTime", "Timestamp too big")
                })
        } else {
            // before unix epoch
            SystemTime::UNIX_EPOCH
                .checked_sub(Duration::from_millis((-millis) as _))
                .ok_or_else(|| {
                    Error::new_from_js_message("Date", "SystemTime", "Timestamp too small")
                })
        }
    }
}

macro_rules! chrono_from_js_impls {
    ($($type:ident;)+) => {
        $(
            #[cfg(feature = "chrono")]
            impl<'js> FromJs<'js> for chrono::DateTime<chrono::$type> {
                fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<chrono::DateTime<chrono::$type>> {
                    use chrono::TimeZone;

                    let millis = date_to_millis(ctx, value)?;

                    chrono::$type.timestamp_millis_opt(millis).single()
                        .ok_or_else(|| {
                            Error::new_from_js_message("Date", "chrono::DateTime", "Invalid timestamp")
                        })
                }
            }
        )+
    };
}

chrono_from_js_impls! {
    Utc;
    Local;
}

#[cfg(test)]
mod test {
    #[test]
    fn js_to_system_time() {
        use crate::{Context, Runtime};
        use std::time::{Duration, SystemTime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();

        ctx.with(|ctx| {
            let res: SystemTime = ctx.eval("new Date(123456789)").unwrap();
            assert_eq!(
                Duration::from_millis(123456789),
                res.duration_since(SystemTime::UNIX_EPOCH).unwrap()
            );

            let res: SystemTime = ctx.eval("new Date(-123456789)").unwrap();
            assert_eq!(
                Duration::from_millis(123456789),
                SystemTime::UNIX_EPOCH.duration_since(res).unwrap()
            );
        });
    }

    #[cfg(feature = "chrono")]
    #[test]
    fn js_to_chrono() {
        use crate::{Context, Runtime};
        use chrono::{DateTime, Utc};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();

        ctx.with(|ctx| {
            let res: DateTime<Utc> = ctx.eval("new Date(123456789)").unwrap();
            assert_eq!(123456789, res.timestamp_millis());
        });

        ctx.with(|ctx| {
            let res: DateTime<Utc> = ctx
                .eval("new Date('Fri Jun 03 2022 23:16:50 GMT+0300')")
                .unwrap();
            assert_eq!(1654287410000, res.timestamp_millis());
        });

        ctx.with(|ctx| {
            let res: DateTime<Utc> = ctx
                .eval("new Date('Fri Jun 03 2022 23:16:50 GMT-0300')")
                .unwrap();
            assert_eq!(1654309010000, res.timestamp_millis());
        });
    }
}
