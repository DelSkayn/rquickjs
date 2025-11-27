//! JavaScript iterable types from Rust iterators.

use crate::{atom::PredefinedAtom, function::MutFn, Ctx, Function, IntoJs, Object, Result, Value};
use core::cell::RefCell;

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
        let iter = RefCell::new(Some(self.0.into_iter()));

        let iterator_fn = Function::new(
            ctx.clone(),
            MutFn::new(move |ctx: Ctx<'js>| -> Result<Object<'js>> {
                let iter_obj = Object::new(ctx.clone())?;
                let iter_taken = iter.borrow_mut().take();

                let state = RefCell::new(iter_taken);
                let next_fn = Function::new(
                    ctx.clone(),
                    MutFn::new(move |ctx: Ctx<'js>| -> Result<Object<'js>> {
                        let result = Object::new(ctx.clone())?;
                        let mut state_ref = state.borrow_mut();

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
}
