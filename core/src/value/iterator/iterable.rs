use std::ops::Deref;

use crate::{
    atom::PredefinedAtom,
    function::{IntoArgs, This},
    Ctx, Error, FromJs, Function, IntoJs, Iterator, Object, Result, Value,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Iterable<'js>(pub(crate) Object<'js>);

impl<'js> Iterable<'js> {
    /// Get the iterator from the iterable.
    ///
    /// This is equivalent to calling `iterable[Symbol.iterator]()` in JavaScript.
    pub fn iterator(&self) -> Result<Iterator<'js>> {
        let iter_fn = self.0.get::<_, Function>(PredefinedAtom::SymbolIterator)?;
        let iterable = (This(self.0.clone()), 2).apply::<Object<'_>>(&iter_fn)?;
        Ok(Iterator(iterable))
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
        if object.is_iterable() {
            Some(Self(object))
        } else {
            None
        }
    }
}

impl<'js> Deref for Iterable<'js> {
    type Target = Object<'js>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'js> AsRef<Object<'js>> for Iterable<'js> {
    fn as_ref(&self) -> &Object<'js> {
        &self.0
    }
}

impl<'js> AsRef<Value<'js>> for Iterable<'js> {
    fn as_ref(&self) -> &Value<'js> {
        self.0.as_ref()
    }
}

impl<'js> From<Iterable<'js>> for Value<'js> {
    fn from(value: Iterable<'js>) -> Self {
        value.into_value()
    }
}

impl<'js> FromJs<'js> for Iterable<'js> {
    fn from_js(_: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        let ty_name = value.type_name();
        if let Some(v) = Self::from_value(value) {
            Ok(v)
        } else {
            Err(Error::new_from_js(ty_name, "Iterable"))
        }
    }
}

impl<'js> IntoJs<'js> for Iterable<'js> {
    fn into_js(self, _ctx: &Ctx<'js>) -> Result<Value<'js>> {
        Ok(self.into_value())
    }
}

impl<'js> Object<'js> {
    /// Returns whether the object is iterable.
    pub fn is_iterable(&self) -> bool {
        self.contains_key(PredefinedAtom::SymbolIterator)
            .unwrap_or(false)
    }

    /// Interpret as [`Iterable`]
    ///
    /// # Safety
    /// You should be sure that the object actually is the required type.
    pub unsafe fn ref_iterable(&self) -> &Iterable<'js> {
        &*(self as *const _ as *const Iterable)
    }

    /// Try reinterpret as [`Iterable`]
    pub fn as_iterable(&self) -> Option<&Iterable<'js>> {
        self.is_iterable().then_some(unsafe { self.ref_iterable() })
    }
}

#[cfg(test)]
mod test {
    use crate::{
        atom::PredefinedAtom,
        function::{Func, This},
        *,
    };

    #[test]
    fn from_javascript() {
        test_with(|ctx| {
            let iterable: Iterable = ctx
                .eval(
                    r#"
                const myIterable = {};
                myIterable[Symbol.iterator] = function* () {
                    yield 1;
                    yield 2;
                    yield 3;
                };
                myIterable
                "#,
                )
                .unwrap();
            let iterator = iterable.iterator().unwrap();
            assert_eq!(iterator.next().unwrap().unwrap().as_int().unwrap(), 1);
            assert_eq!(iterator.next().unwrap().unwrap().as_int().unwrap(), 2);
            assert_eq!(iterator.next().unwrap().unwrap().as_int().unwrap(), 3);
            assert!(iterator.next().unwrap().is_none());
        });
    }

    #[test]
    fn from_rust() {
        fn closure<'js>(ctx: Ctx<'js>) {
            let myiterable = Object::new(ctx.clone()).unwrap();
            myiterable.set("position", 0usize).unwrap();
            myiterable
                .set(
                    PredefinedAtom::SymbolIterator,
                    Func::from(|it: This<Object<'js>>| -> Result<Object<'js>> { Ok(it.0) }),
                )
                .unwrap();
            myiterable
                .set(
                    PredefinedAtom::Next,
                    Func::from(
                        move |ctx: Ctx<'js>, this: This<Object<'js>>| -> Result<Object<'js>> {
                            let position = this.get::<_, usize>("position")?;
                            let res = Object::new(ctx.clone())?;
                            if position >= 3 {
                                res.set(PredefinedAtom::Done, true)?;
                            } else {
                                res.set("value", position)?;
                                this.set("position", position + 1)?;
                            }
                            Ok(res)
                        },
                    ),
                )
                .unwrap();
            let iterator = Iterable::from_object(myiterable)
                .unwrap()
                .iterator()
                .unwrap();
            assert_eq!(iterator.next().unwrap().unwrap().as_int().unwrap(), 0);
            assert_eq!(iterator.next().unwrap().unwrap().as_int().unwrap(), 1);
            assert_eq!(iterator.next().unwrap().unwrap().as_int().unwrap(), 2);
            assert!(iterator.next().unwrap().is_none());
        }

        test_with(closure);
    }
}
