use crate::{
    Array, Ctx, Error, IntoAtom, IntoJs, IteratorJs, Object, Result, StdResult, StdString, String,
    Value,
};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, LinkedList, VecDeque},
    sync::{Mutex, RwLock},
};

#[cfg(feature = "either")]
use either::{Either, Left, Right};

#[cfg(feature = "indexmap")]
use indexmap::{IndexMap, IndexSet};

#[cfg(feature = "chrono")]
impl<'js> IntoJs<'js> for chrono::DateTime<chrono::Utc> {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let global = unsafe { crate::qjs::JS_GetGlobalObject(ctx.ctx) };
        let date_constructor = unsafe {
            crate::qjs::JS_GetPropertyStr(
                ctx.ctx,
                global,
                std::ffi::CStr::from_bytes_with_nul(b"Date\0")
                    .unwrap()
                    .as_ptr(),
            )
        };
        unsafe { crate::qjs::JS_FreeValue(ctx.ctx, global) };

        let f = self.timestamp_millis() as f64;

        let timestamp = crate::qjs::JSValue {
            u: crate::qjs::JSValueUnion { float64: f },
            tag: crate::qjs::JS_TAG_FLOAT64.into(),
        };

        let mut args = vec![timestamp];

        let value = unsafe {
            crate::qjs::JS_CallConstructor(
                ctx.ctx,
                date_constructor,
                args.len() as i32,
                args.as_mut_ptr(),
            )
        };

        unsafe {
            crate::qjs::JS_FreeValue(ctx.ctx, date_constructor);
        }

        Ok(Value { ctx, value })
    }
}

impl<'js> IntoJs<'js> for Value<'js> {
    fn into_js(self, _: Ctx<'js>) -> Result<Value<'js>> {
        Ok(self)
    }
}

impl<'js> IntoJs<'js> for &Value<'js> {
    fn into_js(self, _: Ctx<'js>) -> Result<Value<'js>> {
        Ok(self.clone())
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
            .map_err(Error::from)
            .and_then(|value| value.into_js(ctx))
    }
}

/// Convert the either into JS
#[cfg(feature = "either")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "either")))]
impl<'js, L, R> IntoJs<'js> for Either<L, R>
where
    L: IntoJs<'js>,
    R: IntoJs<'js>,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        match self {
            Left(value) => value.into_js(ctx),
            Right(value) => value.into_js(ctx),
        }
    }
}

/// Convert the either into JS
#[cfg(feature = "either")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "either")))]
impl<'js, L, R> IntoJs<'js> for &Either<L, R>
where
    for<'a> &'a L: IntoJs<'js>,
    for<'a> &'a R: IntoJs<'js>,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        match self {
            Left(value) => value.into_js(ctx),
            Right(value) => value.into_js(ctx),
        }
    }
}

impl<'js, T> IntoJs<'js> for &Box<T>
where
    for<'r> &'r T: IntoJs<'js>,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.as_ref().into_js(ctx)
    }
}

impl<'js, T> IntoJs<'js> for Box<T>
where
    T: IntoJs<'js>,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        (*self).into_js(ctx)
    }
}

impl<'js, T> IntoJs<'js> for &Cell<T>
where
    T: IntoJs<'js> + Copy,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.get().into_js(ctx)
    }
}

impl<'js, T> IntoJs<'js> for &RefCell<T>
where
    for<'r> &'r T: IntoJs<'js>,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.borrow().into_js(ctx)
    }
}

impl<'js, T> IntoJs<'js> for Mutex<T>
where
    T: IntoJs<'js>,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.into_inner().unwrap().into_js(ctx)
    }
}

impl<'js, T> IntoJs<'js> for &Mutex<T>
where
    for<'r> &'r T: IntoJs<'js>,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.lock().unwrap().into_js(ctx)
    }
}

impl<'js, T> IntoJs<'js> for RwLock<T>
where
    T: IntoJs<'js>,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.into_inner().unwrap().into_js(ctx)
    }
}

impl<'js, T> IntoJs<'js> for &RwLock<T>
where
    for<'r> &'r T: IntoJs<'js>,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.read().unwrap().into_js(ctx)
    }
}

macro_rules! into_js_impls {
    // for cells
    (cell: $($type:ident,)*) => {
        $(
            impl<'js, T> IntoJs<'js> for $type<T>
            where
                T: IntoJs<'js>,
            {
                fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
                    self.into_inner().into_js(ctx)
                }
            }
        )*
    };

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
    (list: $($(#[$meta:meta])* $type:ident $({$param:ident})*,)*) => {
        $(
            $(#[$meta])*
            impl<'js, T $(,$param)*> IntoJs<'js> for $type<T $(,$param)*>
            where
                T: IntoJs<'js>,
            {
                fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
                    self.into_iter()
                        .collect_js(ctx)
                        .map(|Array(value)| value)
                }
            }

            $(#[$meta])*
            impl<'js, T $(,$param)*> IntoJs<'js> for &$type<T $(,$param)*>
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
    (map: $($(#[$meta:meta])* $type:ident $({$param:ident})*,)*) => {
        $(
            $(#[$meta])*
            impl<'js, K, V $(,$param)*> IntoJs<'js> for $type<K, V $(,$param)*>
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

            $(#[$meta])*
            impl<'js, K, V $(,$param)*> IntoJs<'js> for &$type<K, V $(,$param)*>
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
    cell:
    Cell,
    RefCell,
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
    /// Convert from Rust vector to JS array
    Vec,
    /// Convert from Rust vector deque to JS array
    VecDeque,
    /// Convert from Rust linked list to JS array
    LinkedList,
    /// Convert from Rust hash set to JS array
    HashSet {S},
    /// Convert from Rust btree set to JS array
    BTreeSet,
    /// Convert from Rust index set to JS array
    #[cfg(feature = "indexmap")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "indexmap")))]
    IndexSet {S},
}

into_js_impls! {
    map:
    /// Convert from Rust hash map to JS object
    HashMap {S},
    /// Convert from Rust btree map to JS object
    BTreeMap,
    /// Convert from Rust index map to JS object
    #[cfg(feature = "indexmap")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "indexmap")))]
    IndexMap {S},
}

into_js_impls! {
    val:
    new_bool: bool,
    new_int: i8 i16 i32 u8 u16,
    new_float: f32 f64,
}

into_js_impls! {
    val:
    i32 f64 => i64 u32 u64 usize isize,
}

mod test {
    #[cfg(feature = "chrono")]
    #[test]
    fn chrono_to_js() {
        use crate::{Context, IntoJs, Runtime};
        use chrono::Utc;

        let ts = Utc::now();
        let millis = ts.timestamp_millis();

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();

        ctx.with(|ctx| {
            let globs = ctx.globals();
            globs.set("ts", ts.into_js(ctx).unwrap()).unwrap();
            let res: i64 = ctx.eval("ts.getTime()").unwrap();
            assert_eq!(millis, res);
        });
    }
}
