use crate::{
    Array, Ctx, Error, IntoAtom, IntoJs, IteratorJs, Object, Result, StdResult, StdString, String,
    Value,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, LinkedList, VecDeque};

impl<'js> IntoJs<'js> for Value<'js> {
    fn into_js(self, _: Ctx<'js>) -> Result<Value<'js>> {
        Ok(self)
    }
}

impl<'js> IntoJs<'js> for StdString {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.as_str().into_js(ctx)
    }
}

impl<'js> IntoJs<'js> for &StdString {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.as_str().into_js(ctx)
    }
}

impl<'js> IntoJs<'js> for &str {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        String::from_str(ctx, self).map(|String(value)| value)
    }
}

impl<'js, T> IntoJs<'js> for &[T]
where
    for<'a> &'a T: IntoJs<'js>,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.iter().collect_js(ctx).map(|Array(value)| value)
    }
}

impl<'js> IntoJs<'js> for () {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        Ok(Value::new_undefined(ctx))
    }
}

impl<'js> IntoJs<'js> for &() {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        Ok(Value::new_undefined(ctx))
    }
}

impl<'js, T> IntoJs<'js> for Option<T>
where
    T: IntoJs<'js>,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        Ok(match self {
            Some(value) => value.into_js(ctx)?,
            _ => Value::new_undefined(ctx),
        })
    }
}

impl<'js, T> IntoJs<'js> for &Option<T>
where
    for<'a> &'a T: IntoJs<'js>,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        Ok(match self {
            Some(value) => value.into_js(ctx)?,
            _ => Value::new_undefined(ctx),
        })
    }
}

impl<'js, T, E> IntoJs<'js> for StdResult<T, E>
where
    T: IntoJs<'js>,
    Error: From<E>,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.map_err(Error::from)
            .and_then(|value| value.into_js(ctx))
    }
}

impl<'js, T, E> IntoJs<'js> for &StdResult<T, E>
where
    for<'a> &'a T: IntoJs<'js>,
    for<'a> Error: From<&'a E>,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.as_ref()
            .map_err(|error| Error::from(error))
            .and_then(|value| value.into_js(ctx))
    }
}

macro_rules! into_js_impls {
    // for tuple types
    (tup: $($($type:ident)*,)*) => {
        $(
            impl<'js, $($type,)*> IntoJs<'js> for ($($type,)*)
            where
                $($type: IntoJs<'js>,)*
            {
                #[allow(non_snake_case)]
                fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
                    let ($($type,)*) = self;
                    let array = Array::new(ctx)?;
                    $(array.set(into_js_impls!(@idx $type), $type)?;)*
                    Ok(array.0)
                }
            }
        )*
    };

    // for list-like Rust types
    (list: $($type:ident,)*) => {
        $(
            impl<'js, T> IntoJs<'js> for $type<T>
            where
                T: IntoJs<'js>,
            {
                fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
                    self.into_iter()
                        .collect_js(ctx)
                        .map(|Array(value)| value)
                }
            }

            impl<'js, T> IntoJs<'js> for &$type<T>
            where
                for<'a> &'a T: IntoJs<'js>,
            {
                fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
                    self.into_iter()
                        .collect_js(ctx)
                        .map(|Array(value)| value)
                }
            }
        )*
    };

    // for map-like Rust types
    (map: $($type:ident,)*) => {
        $(
            impl<'js, K, V> IntoJs<'js> for $type<K, V>
            where
                K: IntoAtom<'js>,
                V: IntoJs<'js>,
            {
                fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
                    self.into_iter()
                        .collect_js(ctx)
                        .map(|Object(value)| value)
                }
            }

            impl<'js, K, V> IntoJs<'js> for &$type<K, V>
            where
                for<'a> &'a K: IntoAtom<'js>,
                for<'a> &'a V: IntoJs<'js>,
            {
                fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
                    self.into_iter()
                        .collect_js(ctx)
                        .map(|Object(value)| value)
                }
            }
        )*
    };

    // for primitive types using `new` function
    (val: $($new:ident: $($type:ident)*,)*) => {
        $(
            $(
                impl<'js> IntoJs<'js> for $type {
                    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
                        Ok(Value::$new(ctx, self as _))
                    }
                }

                impl<'js> IntoJs<'js> for &$type {
                    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
                        (*self).into_js(ctx)
                    }
                }
            )*
        )*
    };

    // for primitive types with two alternatives
    // (ex. u32 may try convert as i32 or else as f64)
    (val: $($alt1:ident $alt2:ident => $($type:ty)*,)*) => {
        $(
            $(
                impl<'js> IntoJs<'js> for $type {
                    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
                        let val = self as $alt1;
                        if val as $type == self {
                            val.into_js(ctx)
                        } else {
                            (self as $alt2).into_js(ctx)
                        }
                    }
                }

                impl<'js> IntoJs<'js> for &$type {
                    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
                        (*self).into_js(ctx)
                    }
                }
            )*
        )*
    };

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

into_js_impls! {
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

into_js_impls! {
    list:
    Vec,
    VecDeque,
    LinkedList,
    HashSet,
    BTreeSet,
}

into_js_impls! {
    map:
    HashMap,
    BTreeMap,
}

into_js_impls! {
    val:
    new_bool: bool,
    new_int: i8 i16 i32 u8 u16,
    new_float: f32 f64,
}

into_js_impls! {
    val:
    i32 f64 => i64 u32 u64,
}
