use std::{iter::Iterator as StdIterator, marker::PhantomData, ops::Deref};

use crate::{
    atom::PredefinedAtom,
    function::{Func, IntoArgs, MutFn, This},
    Ctx, Error, FromJs, Function, IntoJs, Object, Result, Value,
};

mod into_iter;
mod iterable;

pub use into_iter::{IterFn, IterFnMut};
pub use iterable::Iterable;

/// A trait for converting a Rust object into a JavaScript iterator.
pub trait IntoJsIter<'js, I>
where
    I: IntoJs<'js>,
{
    fn next(&mut self, ctx: Ctx<'js>, position: usize) -> Result<Option<I>>;
}

/// A javascript iterator.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Iterator<'js>(pub(crate) Object<'js>);

impl<'js> Iterator<'js> {
    /// Create a new iterable iterator from a Rust object which implements [`IntoJsIter`].
    ///
    pub fn new<T, I>(ctx: Ctx<'js>, mut it: T) -> Result<Self>
    where
        T: IntoJsIter<'js, I> + 'js,
        I: IntoJs<'js>,
    {
        let iterator = Object::new(ctx.clone())?;
        iterator.set("position", 0usize)?;
        iterator.set(
            PredefinedAtom::SymbolIterator,
            Func::from(|it: This<Object<'js>>| -> Result<Object<'js>> { Ok(it.0) }),
        )?;
        iterator.set(
            "next",
            Function::new(
                ctx,
                MutFn::from(
                    move |ctx: Ctx<'js>, this: This<Object<'js>>| -> Result<Object<'js>> {
                        let position = this.get::<_, usize>("position")?;
                        let res = Object::new(ctx.clone())?;
                        if let Some(value) = it.next(ctx, position)? {
                            res.set("value", value)?;
                            this.set("position", position + 1)?;
                        } else {
                            res.set(PredefinedAtom::Done, true)?;
                        }
                        Ok(res)
                    },
                ),
            ),
        )?;

        Ok(Self(iterator))
    }

    /// Get the next value from the iterator.
    pub fn next(&self) -> Result<Option<Value<'js>>> {
        let next_fn = self.0.get::<_, Function>(PredefinedAtom::Next)?;
        let next = (This(self.0.clone()), 2).apply::<Object<'_>>(&next_fn)?;
        if let Ok(done) = next.get::<_, bool>(PredefinedAtom::Done) {
            if done {
                return Ok(None);
            }
        }
        let value = next.get::<_, Value<'_>>("value")?;
        Ok(Some(value))
    }

    /// Reference to value
    #[inline]
    pub fn as_value(&self) -> &Value<'js> {
        self.0.as_value()
    }

    /// Convert into value
    #[inline]
    pub fn into_value(self) -> Value<'js> {
        self.0.into_value()
    }

    /// Convert from value
    pub fn from_value(value: Value<'js>) -> Option<Self> {
        Self::from_object(Object::from_value(value).ok()?)
    }

    /// Reference as an object
    #[inline]
    pub fn as_object(&self) -> &Object<'js> {
        &self.0
    }

    /// Convert into an object
    #[inline]
    pub fn into_object(self) -> Object<'js> {
        self.0
    }

    /// Convert from an object
    pub fn from_object(object: Object<'js>) -> Option<Self> {
        if object.is_iterator() {
            Some(Self(object))
        } else {
            None
        }
    }
}

impl<'js> Deref for Iterator<'js> {
    type Target = Object<'js>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'js> AsRef<Object<'js>> for Iterator<'js> {
    fn as_ref(&self) -> &Object<'js> {
        &self.0
    }
}

impl<'js> AsRef<Value<'js>> for Iterator<'js> {
    fn as_ref(&self) -> &Value<'js> {
        self.0.as_ref()
    }
}

impl<'js> From<Iterator<'js>> for Value<'js> {
    fn from(value: Iterator<'js>) -> Self {
        value.into_value()
    }
}

impl<'js> FromJs<'js> for Iterator<'js> {
    fn from_js(_: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let ty_name = value.type_name();
        if let Some(v) = Self::from_value(value) {
            Ok(v)
        } else {
            Err(Error::new_from_js(ty_name, "Iterator"))
        }
    }
}

impl<'js> IntoJs<'js> for Iterator<'js> {
    fn into_js(self, _ctx: &Ctx<'js>) -> Result<Value<'js>> {
        Ok(self.into_value())
    }
}

impl<'js> Object<'js> {
    /// Returns whether the object is an iterator.
    pub fn is_iterator(&self) -> bool {
        self.get::<_, Function>("next").is_ok()
    }

    /// Interpret as [`Iterator`]
    ///
    /// # Safety
    /// You should be sure that the object actually is the required type.
    pub unsafe fn ref_iterator(&self) -> &Iterator<'js> {
        &*(self as *const _ as *const Iterator)
    }

    /// Try reinterpret as [`Iterator`]
    pub fn as_iterator(&self) -> Option<&Iterator<'js>> {
        self.is_iterator().then_some(unsafe { self.ref_iterator() })
    }
}

/// A rust iterator over the values of a js iterator.
pub struct IteratorIter<'js, T> {
    iterator: Iterator<'js>,
    marker: PhantomData<T>,
}

impl<'js, T> StdIterator for IteratorIter<'js, T>
where
    T: FromJs<'js>,
{
    type Item = Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.iterator.next().transpose()?.ok()?;
        Some(T::from_js(self.iterator.ctx(), next))
    }
}

impl<'js> IntoIterator for Iterator<'js> {
    type Item = Result<Value<'js>>;
    type IntoIter = IteratorIter<'js, Value<'js>>;

    fn into_iter(self) -> Self::IntoIter {
        IteratorIter {
            iterator: self,
            marker: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn js_iterator_from_rust() {
        test_with(|ctx| {
            let iterator: Iterator = ctx
                .eval(
                    r#"
                const array = ['a', 'b', 'c'];
                const iterator = array[Symbol.iterator]();
                iterator
                "#,
                )
                .unwrap();
            assert_eq!(
                iterator
                    .next()
                    .unwrap()
                    .unwrap()
                    .as_string()
                    .unwrap()
                    .to_string()
                    .unwrap(),
                "a"
            );
            assert_eq!(
                iterator
                    .next()
                    .unwrap()
                    .unwrap()
                    .as_string()
                    .unwrap()
                    .to_string()
                    .unwrap(),
                "b"
            );
            assert_eq!(
                iterator
                    .next()
                    .unwrap()
                    .unwrap()
                    .as_string()
                    .unwrap()
                    .to_string()
                    .unwrap(),
                "c"
            );
            assert!(iterator.next().unwrap().is_none());
        });
    }

    #[test]
    fn js_iterator_from_rust_iter() {
        test_with(|ctx| {
            let values = ctx
                .eval::<Iterator, _>(
                    r#"
                const array = ['a', 'b', 'c'];
                const iterator = array[Symbol.iterator]();
                iterator
                "#,
                )
                .unwrap()
                .into_iter()
                .collect::<Result<Vec<_>>>()
                .unwrap();
            assert_eq!(values.len(), 3);
            assert_eq!(values[0].as_string().unwrap().to_string().unwrap(), "a");
            assert_eq!(values[1].as_string().unwrap().to_string().unwrap(), "b");
            assert_eq!(values[2].as_string().unwrap().to_string().unwrap(), "c");
        });
    }

    #[test]
    fn rust_iterator_from_js() {
        test_with(|ctx| {
            let iterator = Iterator::new(
                ctx.clone(),
                IterFn::from(|_, position| {
                    if position < 3 {
                        Ok(Some(position))
                    } else {
                        Ok(None)
                    }
                }),
            )
            .unwrap();
            ctx.globals().set("myiterator", iterator).unwrap();
            let res: String = ctx
                .eval(
                    r#"
                    const res = [];
                    for (let i of myiterator) {
                        res.push(i);
                    }
                    res.join(',')
                "#,
                )
                .unwrap();
            assert_eq!(res.to_string().unwrap(), "0,1,2");
        });
    }

    #[test]
    fn rust_iterator_from_rust() {
        test_with(|ctx| {
            let iterator = Iterator::new(
                ctx.clone(),
                IterFn::from(|_, position| {
                    if position < 3 {
                        Ok(Some(position))
                    } else {
                        Ok(None)
                    }
                }),
            )
            .unwrap();
            assert_eq!(iterator.next().unwrap().unwrap().as_int().unwrap(), 0);
            assert_eq!(iterator.next().unwrap().unwrap().as_int().unwrap(), 1);
            assert_eq!(iterator.next().unwrap().unwrap().as_int().unwrap(), 2);
            assert!(iterator.next().unwrap().is_none());
        });
    }

    #[test]
    fn rust_iterator_trait() {
        test_with(|ctx| {
            let data = vec![1, 2, 3];
            let iterator = Iterator::new(ctx.clone(), data.into_iter()).unwrap();
            ctx.globals().set("myiterator", iterator).unwrap();
            let res: String = ctx
                .eval(
                    r#"
                    const res = [];
                    for (let i of myiterator) {
                        res.push(i);
                    }
                    res.join(',')
                "#,
                )
                .unwrap();
            assert_eq!(res.to_string().unwrap(), "1,2,3");
        });
    }
}
