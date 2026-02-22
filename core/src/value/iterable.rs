//! JavaScript iterator and iterable types.
//!
//! - [`Iterable`] — wraps a Rust closure or iterator as a JS iterator
//! - [`JsIterator`] — consumes a JS iterator from Rust

use crate::js_lifetime::JsLifetime;
use crate::object::Property;
use crate::safe_ref::Mut;
use crate::{
    atom::PredefinedAtom,
    function::{MutFn, This},
    Array, Ctx, Error, FromJs, Function, IntoJs, Object, Result, Value,
};
use core::{iter::FusedIterator, marker::PhantomData};

struct IteratorPrototypeCache<'js>(Object<'js>);

unsafe impl<'js> JsLifetime<'js> for IteratorPrototypeCache<'js> {
    type Changed<'to> = IteratorPrototypeCache<'to>;
}

fn get_iterator_prototype<'js>(ctx: &Ctx<'js>) -> Result<Object<'js>> {
    if let Some(guard) = ctx.userdata::<IteratorPrototypeCache>() {
        return Ok(guard.0.clone());
    }

    let array = Array::new(ctx.clone())?;
    let iter_fn: Function = array.as_object().get(PredefinedAtom::SymbolIterator)?;
    let array_iter: Object = iter_fn.call((This(array),))?;
    let array_iter_proto = array_iter
        .get_prototype()
        .ok_or_else(|| Error::new_from_js("value", "iterator prototype"))?;
    let proto = array_iter_proto
        .get_prototype()
        .ok_or_else(|| Error::new_from_js("value", "iterator prototype"))?;

    let _ = ctx.store_userdata(IteratorPrototypeCache(proto.clone()));

    Ok(proto)
}

fn build_iterator_object<'js, F>(ctx: &Ctx<'js>, next_fn: F) -> Result<Object<'js>>
where
    F: FnMut(&Ctx<'js>) -> Option<Result<Value<'js>>> + 'js,
{
    let iter_proto = get_iterator_prototype(ctx)?;

    let proto = Object::new(ctx.clone())?;
    proto.set_prototype(Some(&iter_proto))?;

    let state = Mut::new(next_fn);
    let next = Function::new(
        ctx.clone(),
        MutFn::new(move |ctx: Ctx<'js>| -> Result<Object<'js>> {
            let result = Object::new(ctx.clone())?;
            match (state.lock())(&ctx) {
                Some(Ok(value)) => {
                    result.set(PredefinedAtom::Value, value)?;
                    result.set(PredefinedAtom::Done, false)?;
                }
                Some(Err(e)) => return Err(e),
                None => {
                    result.set(PredefinedAtom::Done, true)?;
                }
            }
            Ok(result)
        }),
    )?;

    proto.prop(
        PredefinedAtom::Next,
        Property::from(next).enumerable().writable().configurable(),
    )?;

    let iter_obj = Object::new(ctx.clone())?;
    iter_obj.set_prototype(Some(&proto))?;

    Ok(iter_obj)
}

/// Creates a JavaScript iterator from a Rust closure or iterator.
///
/// The returned JS object has `next()` and inherits
/// `[Symbol.iterator]() { return this }` from `%IteratorPrototype%`,
/// so it works with `for...of`, spread, and destructuring.
///
/// # From a closure
/// ```
/// # use rquickjs::{Runtime, Context, Result, Iterable};
/// # let rt = Runtime::new().unwrap();
/// # let ctx = Context::full(&rt).unwrap();
/// # ctx.with(|ctx| -> Result<()> {
/// let mut i = 0;
/// let iter = Iterable::from_fn(move || {
///     i += 1;
///     if i <= 3 { Some(i) } else { None }
/// });
/// ctx.globals().set("myIter", iter)?;
/// let result: Vec<i32> = ctx.eval("[...myIter]")?;
/// assert_eq!(result, vec![1, 2, 3]);
/// # Ok(())
/// # }).unwrap();
/// ```
///
/// # From an iterator
/// ```
/// # use rquickjs::{Runtime, Context, Result, Iterable};
/// # let rt = Runtime::new().unwrap();
/// # let ctx = Context::full(&rt).unwrap();
/// # ctx.with(|ctx| -> Result<()> {
/// let iter = Iterable::from(vec![1, 2, 3]);
/// ctx.globals().set("myIter", iter)?;
/// let result: Vec<i32> = ctx.eval("[...myIter]")?;
/// assert_eq!(result, vec![1, 2, 3]);
/// # Ok(())
/// # }).unwrap();
/// ```
pub struct Iterable<F>(F);

impl<F> Iterable<F> {
    /// Create from a `FnMut() -> Option<T>` closure.
    pub fn from_fn(f: F) -> Self {
        Iterable(f)
    }
}

impl<I: IntoIterator> From<I> for Iterable<IteratorWrapper<I::IntoIter>> {
    fn from(iter: I) -> Self {
        Iterable(IteratorWrapper(Some(iter.into_iter())))
    }
}

/// Wrapper that adapts a Rust `Iterator` into a `FnMut() -> Option<T>`.
pub struct IteratorWrapper<I>(Option<I>);

impl<I: Iterator> IteratorWrapper<I> {
    fn next(&mut self) -> Option<I::Item> {
        self.0.as_mut()?.next()
    }
}

impl<'js, F, T> IntoJs<'js> for Iterable<F>
where
    F: FnMut() -> Option<T> + 'js,
    T: IntoJs<'js> + 'js,
{
    fn into_js(self, ctx: &Ctx<'js>) -> Result<Value<'js>> {
        let mut f = self.0;
        let iter_obj = build_iterator_object(ctx, move |ctx| f().map(|v| v.into_js(ctx)))?;
        Ok(iter_obj.into_value())
    }
}

impl<'js, I, T> IntoJs<'js> for Iterable<IteratorWrapper<I>>
where
    I: Iterator<Item = T> + 'js,
    T: IntoJs<'js> + 'js,
{
    fn into_js(self, ctx: &Ctx<'js>) -> Result<Value<'js>> {
        let mut w = self.0;
        let iter_obj = build_iterator_object(ctx, move |ctx| w.next().map(|v| v.into_js(ctx)))?;
        Ok(iter_obj.into_value())
    }
}

/// Consumes a JavaScript iterator from Rust.
///
/// Wraps a JS iterator object and implements Rust's [`Iterator`] trait.
/// Can be created from any JS iterable (arrays, maps, sets, generators, etc.).
///
/// # Example
/// ```
/// # use rquickjs::{Runtime, Context, Result, JsIterator};
/// # let rt = Runtime::new().unwrap();
/// # let ctx = Context::full(&rt).unwrap();
/// # ctx.with(|ctx| -> Result<()> {
/// let iter: JsIterator<i32> = ctx.eval("[1, 2, 3]")?;
/// let values: Vec<i32> = iter.filter_map(|r| r.ok()).collect();
/// assert_eq!(values, vec![1, 2, 3]);
/// # Ok(())
/// # }).unwrap();
/// ```
pub struct JsIterator<'js, T = Value<'js>> {
    iterator: Object<'js>,
    done: bool,
    _marker: PhantomData<T>,
}

impl<'js, T> JsIterator<'js, T> {
    /// Returns the underlying JS iterator object.
    pub fn into_inner(self) -> Object<'js> {
        self.iterator
    }

    /// Maps this iterator to yield a different type.
    ///
    /// This is useful when you have a `JsIterator<Value>` and want to convert
    /// values to a specific type.
    pub fn typed<U: FromJs<'js>>(self) -> JsIterator<'js, U> {
        JsIterator {
            iterator: self.iterator,
            done: self.done,
            _marker: PhantomData,
        }
    }
}

impl<'js, T: FromJs<'js>> Iterator for JsIterator<'js, T> {
    type Item = Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let next_fn: Function<'js> = match self.iterator.get(PredefinedAtom::Next) {
            Ok(f) => f,
            Err(e) => return Some(Err(e)),
        };

        let result: Object<'js> = match next_fn.call((This(self.iterator.clone()),)) {
            Ok(r) => r,
            Err(e) => return Some(Err(e)),
        };

        let done: bool = match result.get(PredefinedAtom::Done) {
            Ok(d) => d,
            Err(e) => return Some(Err(e)),
        };

        if done {
            self.done = true;
            return None;
        }

        let value: Value<'js> = match result.get(PredefinedAtom::Value) {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        };

        Some(T::from_js(self.iterator.ctx(), value))
    }
}

impl<'js, T: FromJs<'js>> FusedIterator for JsIterator<'js, T> {}

impl<'js, T: FromJs<'js>> FromJs<'js> for JsIterator<'js, T> {
    fn from_js(_ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let obj = Object::from_value(value)?;

        // Try Symbol.iterator first (for iterables like arrays)
        if let Ok(iter_fn) = obj.get::<_, Function<'js>>(PredefinedAtom::SymbolIterator) {
            let iterator: Object<'js> = iter_fn.call((This(obj),))?;
            return Ok(JsIterator {
                iterator,
                done: false,
                _marker: PhantomData,
            });
        }

        // Fall back to treating it as an iterator (has `next` method)
        if obj.contains_key(PredefinedAtom::Next)? {
            return Ok(JsIterator {
                iterator: obj,
                done: false,
                _marker: PhantomData,
            });
        }

        Err(Error::new_from_js(
            "value",
            "iterable (object with Symbol.iterator or next)",
        ))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::*;

    #[test]
    fn from_vec() {
        test_with(|ctx| {
            let iter = Iterable::from(vec![1i32, 2, 3]);
            ctx.globals().set("myIter", iter).unwrap();
            let result: Vec<i32> = ctx.eval("[...myIter]").unwrap();
            assert_eq!(result, vec![1, 2, 3]);
        });
    }

    #[test]
    fn from_range() {
        test_with(|ctx| {
            let iter = Iterable::from(0..5);
            ctx.globals().set("myIter", iter).unwrap();
            let result: Vec<i32> = ctx.eval("[...myIter]").unwrap();
            assert_eq!(result, vec![0, 1, 2, 3, 4]);
        });
    }

    #[test]
    fn from_closure() {
        test_with(|ctx| {
            let mut i = 0i32;
            let iter = Iterable::from_fn(move || {
                i += 1;
                if i <= 3 {
                    Some(i)
                } else {
                    None
                }
            });
            ctx.globals().set("myIter", iter).unwrap();
            let result: Vec<i32> = ctx.eval("[...myIter]").unwrap();
            assert_eq!(result, vec![1, 2, 3]);
        });
    }

    #[test]
    fn for_of() {
        test_with(|ctx| {
            let iter = Iterable::from(vec!["a", "b", "c"]);
            ctx.globals().set("myIter", iter).unwrap();
            let result: alloc::string::String = ctx
                .eval(r#"let s = ""; for (const x of myIter) { s += x; } s"#)
                .unwrap();
            assert_eq!(result, "abc");
        });
    }

    #[test]
    fn symbol_iterator_returns_this() {
        test_with(|ctx| {
            let iter = Iterable::from(vec![1i32]);
            ctx.globals().set("myIter", iter).unwrap();
            let ok: bool = ctx.eval("myIter[Symbol.iterator]() === myIter").unwrap();
            assert!(ok);
        });
    }

    #[test]
    fn prototype_chain() {
        test_with(|ctx| {
            let iter = Iterable::from(vec![1i32]);
            ctx.globals().set("myIter", iter).unwrap();
            let ok: bool = ctx
                .eval(
                    r#"
                    const iterProto = Object.getPrototypeOf(
                        Object.getPrototypeOf([][Symbol.iterator]())
                    );
                    Object.getPrototypeOf(Object.getPrototypeOf(myIter)) === iterProto
                    "#,
                )
                .unwrap();
            assert!(ok);
        });
    }

    #[test]
    fn next_descriptors() {
        test_with(|ctx| {
            let iter = Iterable::from(vec![1i32]);
            ctx.globals().set("myIter", iter).unwrap();
            let ok: bool = ctx
                .eval(
                    r#"
                    const proto = Object.getPrototypeOf(myIter);
                    const desc = Object.getOwnPropertyDescriptor(proto, "next");
                    desc.enumerable && desc.writable && desc.configurable
                    "#,
                )
                .unwrap();
            assert!(ok);
        });
    }

    #[test]
    fn js_iter_from_array() {
        test_with(|ctx| {
            let iter: JsIterator<i32> = ctx.eval("[1, 2, 3][Symbol.iterator]()").unwrap();
            let values: Vec<i32> = iter.filter_map(|r| r.ok()).collect();
            assert_eq!(values, vec![1, 2, 3]);
        });
    }

    #[test]
    fn js_iter_from_iterable() {
        test_with(|ctx| {
            let iter: JsIterator<i32> = ctx.eval("[4, 5, 6]").unwrap();
            let values: Vec<i32> = iter.filter_map(|r| r.ok()).collect();
            assert_eq!(values, vec![4, 5, 6]);
        });
    }

    #[test]
    fn js_iter_from_generator() {
        test_with(|ctx| {
            let iter: JsIterator<i32> = ctx
                .eval("(function*() { yield 10; yield 20; yield 30; })()")
                .unwrap();
            let values: Vec<i32> = iter.filter_map(|r| r.ok()).collect();
            assert_eq!(values, vec![10, 20, 30]);
        });
    }

    #[test]
    fn js_iter_roundtrip() {
        test_with(|ctx| {
            let rust_iter = Iterable::from(vec![100i32, 200, 300]);
            ctx.globals().set("myIter", rust_iter).unwrap();
            let js_iter: JsIterator<i32> = ctx.eval("myIter").unwrap();
            let values: Vec<i32> = js_iter.filter_map(|r| r.ok()).collect();
            assert_eq!(values, vec![100, 200, 300]);
        });
    }

    #[test]
    fn js_iter_raw_values() {
        test_with(|ctx| {
            let iter: JsIterator<Value> = ctx.eval("[1, 'two', 3]").unwrap();
            let values: Vec<Value> = iter.filter_map(|r| r.ok()).collect();
            assert_eq!(values.len(), 3);
        });
    }

    #[test]
    fn js_iter_typed() {
        test_with(|ctx| {
            let iter: JsIterator<Value> = ctx.eval("[1, 2, 3]").unwrap();
            let values: Vec<i32> = iter.typed::<i32>().filter_map(|r| r.ok()).collect();
            assert_eq!(values, vec![1, 2, 3]);
        });
    }

    #[test]
    fn js_iter_map_entries() {
        test_with(|ctx| {
            let iter: JsIterator<Array> =
                ctx.eval("new Map([['a', 1], ['b', 2]]).entries()").unwrap();
            let entries: Vec<Array> = iter.filter_map(|r| r.ok()).collect();
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0].get::<alloc::string::String>(0).unwrap(), "a");
            assert_eq!(entries[0].get::<i32>(1).unwrap(), 1);
        });
    }

    #[test]
    fn js_iter_set() {
        test_with(|ctx| {
            let iter: JsIterator<i32> = ctx.eval("new Set([1, 2, 3])").unwrap();
            let values: Vec<i32> = iter.filter_map(|r| r.ok()).collect();
            assert_eq!(values, vec![1, 2, 3]);
        });
    }
}
