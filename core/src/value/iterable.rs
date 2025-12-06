//! JavaScript iterable types from Rust iterators.

use crate::safe_ref::Mut;
use crate::{
    atom::PredefinedAtom,
    function::{MutFn, This},
    Ctx, Error, FromJs, Function, IntoJs, Object, Result, Value,
};
use core::{iter::FusedIterator, marker::PhantomData};

/// Converts a Rust iterator into a JavaScript iterable object.
///
/// The resulting object implements the JavaScript iterable protocol with a
/// `[Symbol.iterator]` method that returns an iterator following the iterator protocol.
///
/// Note: The iterator can only be consumed once. Subsequent iterations will yield no values.
///
/// # Example
/// ```
/// # use rquickjs::{Runtime, Context, Result, Iterable};
/// # let rt = Runtime::new().unwrap();
/// # let ctx = Context::full(&rt).unwrap();
/// # ctx.with(|ctx| -> Result<()> {
/// // Create an iterable from a Vec
/// let iter = Iterable::from(vec![1, 2, 3]);
/// ctx.globals().set("myIterable", iter)?;
///
/// // Use spread operator
/// let result: Vec<i32> = ctx.eval("[...myIterable]")?;
/// assert_eq!(result, vec![1, 2, 3]);
/// # Ok(())
/// # }).unwrap();
/// ```
pub struct Iterable<I>(pub I);

impl<I> From<I> for Iterable<I> {
    fn from(iter: I) -> Self {
        Iterable(iter)
    }
}

impl<'js, I, T> IntoJs<'js> for Iterable<I>
where
    I: IntoIterator<Item = T> + 'js,
    I::IntoIter: 'js,
    T: IntoJs<'js> + 'js,
{
    fn into_js(self, ctx: &Ctx<'js>) -> Result<Value<'js>> {
        let iter = Mut::new(Some(self.0.into_iter()));

        let iterator_fn = Function::new(
            ctx.clone(),
            MutFn::new(move |ctx: Ctx<'js>| -> Result<Object<'js>> {
                let iter_obj = Object::new(ctx.clone())?;
                let iter_taken = iter.lock().take();

                let state = Mut::new(iter_taken);
                let next_fn = Function::new(
                    ctx.clone(),
                    MutFn::new(move |ctx: Ctx<'js>| -> Result<Object<'js>> {
                        let result = Object::new(ctx.clone())?;
                        let mut state_ref = state.lock();

                        if let Some(ref mut it) = *state_ref {
                            if let Some(value) = it.next() {
                                result.set(PredefinedAtom::Value, value.into_js(&ctx)?)?;
                                result.set(PredefinedAtom::Done, false)?;
                            } else {
                                result.set(PredefinedAtom::Done, true)?;
                                *state_ref = None;
                            }
                        } else {
                            result.set(PredefinedAtom::Done, true)?;
                        }
                        Ok(result)
                    }),
                )?;

                iter_obj.set(PredefinedAtom::Next, next_fn)?;
                Ok(iter_obj)
            }),
        )?;

        let obj = Object::new(ctx.clone())?;
        obj.set(PredefinedAtom::SymbolIterator, iterator_fn)?;
        Ok(obj.into_value())
    }
}

/// An iterator over values from a JavaScript iterable.
///
/// This struct wraps a JavaScript iterator object and implements Rust's `Iterator` trait,
/// allowing you to consume JavaScript iterables from Rust code.
///
/// The type parameter `T` specifies what type each value should be converted to.
/// Use `Value<'js>` to get raw JS values without conversion.
///
/// # Example
/// ```
/// # use rquickjs::{Runtime, Context, Result, JsIterator, Value};
/// # let rt = Runtime::new().unwrap();
/// # let ctx = Context::full(&rt).unwrap();
/// # ctx.with(|ctx| -> Result<()> {
/// // Get an iterator with automatic conversion to i32
/// let iter: JsIterator<i32> = ctx.eval("[1, 2, 3]")?;
/// let values: Vec<i32> = iter.filter_map(|r| r.ok()).collect();
/// assert_eq!(values, vec![1, 2, 3]);
///
/// // Get raw JS values without conversion
/// let iter: JsIterator<Value> = ctx.eval("['a', 'b']")?;
/// for value in iter {
///     println!("{:?}", value?);
/// }
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
    fn iterable_spread() {
        test_with(|ctx| {
            let iter = Iterable::from(vec![1i32, 2, 3]);
            ctx.globals().set("myIter", iter).unwrap();
            let result: Vec<i32> = ctx.eval("[...myIter]").unwrap();
            assert_eq!(result, vec![1, 2, 3]);
        });
    }

    #[test]
    fn iterable_for_of() {
        test_with(|ctx| {
            let iter = Iterable::from(vec!["a", "b", "c"]);
            ctx.globals().set("myIter", iter).unwrap();
            let result: alloc::string::String = ctx
                .eval(
                    r#"
                let s = "";
                for (const x of myIter) { s += x; }
                s
            "#,
                )
                .unwrap();
            assert_eq!(result, "abc");
        });
    }

    #[test]
    fn iterable_from_range() {
        test_with(|ctx| {
            let iter = Iterable::from(0..5);
            ctx.globals().set("myIter", iter).unwrap();
            let result: Vec<i32> = ctx.eval("[...myIter]").unwrap();
            assert_eq!(result, vec![0, 1, 2, 3, 4]);
        });
    }

    #[test]
    fn iterable_single_use() {
        test_with(|ctx| {
            let iter = Iterable::from(vec![1i32, 2]);
            ctx.globals().set("myIter", iter).unwrap();
            // First iteration consumes the iterator
            let first: Vec<i32> = ctx.eval("[...myIter]").unwrap();
            assert_eq!(first, vec![1, 2]);
            // Second iteration returns empty (iterator exhausted)
            let second: Vec<i32> = ctx.eval("[...myIter]").unwrap();
            assert_eq!(second, Vec::<i32>::new());
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
            // Pass an iterable (array), not an iterator
            let iter: JsIterator<i32> = ctx.eval("[4, 5, 6]").unwrap();
            let values: Vec<i32> = iter.filter_map(|r| r.ok()).collect();
            assert_eq!(values, vec![4, 5, 6]);
        });
    }

    #[test]
    fn js_iter_from_generator() {
        test_with(|ctx| {
            let iter: JsIterator<i32> = ctx
                .eval(
                    r#"
                (function*() {
                    yield 10;
                    yield 20;
                    yield 30;
                })()
            "#,
                )
                .unwrap();
            let values: Vec<i32> = iter.filter_map(|r| r.ok()).collect();
            assert_eq!(values, vec![10, 20, 30]);
        });
    }

    #[test]
    fn js_iter_roundtrip() {
        test_with(|ctx| {
            // Rust -> JS -> Rust roundtrip
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
            // Get raw Value without conversion
            let iter: JsIterator<Value> = ctx.eval("[1, 'two', 3]").unwrap();
            let values: Vec<Value> = iter.filter_map(|r| r.ok()).collect();
            assert_eq!(values.len(), 3);
            assert!(values[0].is_int());
            assert!(values[1].is_string());
            assert!(values[2].is_int());
        });
    }

    #[test]
    fn js_iter_typed_conversion() {
        test_with(|ctx| {
            // Start with raw values, then convert
            let iter: JsIterator<Value> = ctx.eval("[1, 2, 3]").unwrap();
            let typed = iter.typed::<i32>();
            let values: Vec<i32> = typed.filter_map(|r| r.ok()).collect();
            assert_eq!(values, vec![1, 2, 3]);
        });
    }

    #[test]
    fn js_iter_strings() {
        test_with(|ctx| {
            let iter: JsIterator<alloc::string::String> =
                ctx.eval("['hello', 'world', 'rust']").unwrap();
            let values: Vec<alloc::string::String> = iter.filter_map(|r| r.ok()).collect();
            assert_eq!(values, vec!["hello", "world", "rust"]);
        });
    }

    #[test]
    fn js_iter_floats() {
        test_with(|ctx| {
            let iter: JsIterator<f64> = ctx.eval("[1.5, 2.7, 3.54]").unwrap();
            let values: Vec<f64> = iter.filter_map(|r| r.ok()).collect();
            assert_eq!(values, vec![1.5, 2.7, 3.54]);
        });
    }

    #[test]
    fn js_iter_bools() {
        test_with(|ctx| {
            let iter: JsIterator<bool> = ctx.eval("[true, false, true]").unwrap();
            let values: Vec<bool> = iter.filter_map(|r| r.ok()).collect();
            assert_eq!(values, vec![true, false, true]);
        });
    }

    #[test]
    fn js_iter_objects() {
        test_with(|ctx| {
            let iter: JsIterator<Object> = ctx.eval("[{a: 1}, {b: 2}]").unwrap();
            let objects: Vec<Object> = iter.filter_map(|r| r.ok()).collect();
            assert_eq!(objects.len(), 2);
            assert_eq!(objects[0].get::<_, i32>("a").unwrap(), 1);
            assert_eq!(objects[1].get::<_, i32>("b").unwrap(), 2);
        });
    }

    #[test]
    fn js_iter_map_entries() {
        test_with(|ctx| {
            // Map.entries() returns [key, value] pairs
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
