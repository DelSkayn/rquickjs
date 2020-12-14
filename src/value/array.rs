use crate::{
    get_exception, handle_exception, qjs, Ctx, Error, FromIteratorJs, FromJs, IntoJs, Object,
    Result, Value,
};
use std::{
    iter::{DoubleEndedIterator, ExactSizeIterator, FusedIterator, IntoIterator, Iterator},
    marker::PhantomData,
};

/// Rust representation of a javascript object optimized as an array.
///
/// Javascript array's are objects and can be used as such.
/// However arrays in quickjs are optimized when they do not have any holes.
/// This value represents such a optimized array.
#[derive(Debug, PartialEq, Clone)]
#[repr(transparent)]
pub struct Array<'js>(pub(crate) Value<'js>);

impl<'js> Array<'js> {
    pub fn new(ctx: Ctx<'js>) -> Result<Self> {
        Ok(Array(unsafe {
            let val = qjs::JS_NewArray(ctx.ctx);
            handle_exception(ctx, val)?;
            Value::from_js_value(ctx, val)
        }))
    }

    /// Get the lenght of the javascript array.
    pub fn len(&self) -> usize {
        let ctx = self.0.ctx;
        let value = self.0.as_js_value();
        unsafe {
            let val = qjs::JS_GetPropertyStr(ctx.ctx, value, b"length\0".as_ptr() as *const _);
            assert!(qjs::JS_IsInt(val));
            qjs::JS_VALUE_GET_INT(val) as _
        }
    }

    /// Returns wether a javascript array is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the value at an index in the javascript array.
    pub fn get<V: FromJs<'js>>(&self, idx: usize) -> Result<V> {
        let ctx = self.0.ctx;
        let obj = self.0.as_js_value();
        let val = unsafe {
            let val = qjs::JS_GetPropertyUint32(ctx.ctx, obj, idx as _);
            let val = handle_exception(ctx, val)?;
            Value::from_js_value(ctx, val)
        };
        V::from_js(ctx, val)
    }

    /// Set the value at an index in the javascript array.
    pub fn set<V: IntoJs<'js>>(&self, idx: usize, val: V) -> Result<()> {
        let ctx = self.0.ctx;
        let obj = self.0.as_js_value();
        let val = val.into_js(ctx)?.into_js_value();
        unsafe {
            if 0 > qjs::JS_SetPropertyUint32(ctx.ctx, obj, idx as _, val) {
                return Err(get_exception(ctx));
            }
        }
        Ok(())
    }

    /// Get iterator over elments of an array
    pub fn iter<T: FromJs<'js>>(&self) -> ArrayIter<'js, T> {
        let count = self.len() as _;
        ArrayIter {
            array: self.clone(),
            index: 0,
            count,
            marker: PhantomData,
        }
    }

    /// Reference as an object
    #[inline]
    pub fn as_object(&self) -> &Object<'js> {
        unsafe { &*(self as *const _ as *const Object) }
    }

    /// Convert into an object
    #[inline]
    pub fn into_object(self) -> Object<'js> {
        Object(self.0)
    }

    /// Convert from an object
    pub fn from_object(object: Object<'js>) -> Result<Self> {
        if object.is_array() {
            Ok(Self(object.0))
        } else {
            Err(Error::new_from_js("object", "array"))
        }
    }
}

/// The iterator for an array
pub struct ArrayIter<'js, T> {
    array: Array<'js>,
    index: u32,
    count: u32,
    marker: PhantomData<T>,
}

impl<'js, T> Iterator for ArrayIter<'js, T>
where
    T: FromJs<'js>,
{
    type Item = Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.count {
            let res = self.array.get(self.index as _);
            self.index += 1;
            Some(res)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = (self.count - self.index) as _;
        (len, Some(len))
    }
}

impl<'js, T> DoubleEndedIterator for ArrayIter<'js, T>
where
    T: FromJs<'js>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.index < self.count {
            self.count -= 1;
            let res = self.array.get(self.count as _);
            Some(res)
        } else {
            None
        }
    }
}

impl<'js, T> ExactSizeIterator for ArrayIter<'js, T> where T: FromJs<'js> {}

impl<'js, T> FusedIterator for ArrayIter<'js, T> where T: FromJs<'js> {}

impl<'js> IntoIterator for Array<'js> {
    type Item = Result<Value<'js>>;
    type IntoIter = ArrayIter<'js, Value<'js>>;

    fn into_iter(self) -> Self::IntoIter {
        let count = self.len() as _;
        ArrayIter {
            array: self,
            index: 0,
            count,
            marker: PhantomData,
        }
    }
}

impl<'js, A> FromIteratorJs<'js, A> for Array<'js>
where
    A: IntoJs<'js>,
{
    type Item = Value<'js>;

    fn from_iter_js<T>(ctx: Ctx<'js>, iter: T) -> Result<Self>
    where
        T: IntoIterator<Item = A>,
    {
        let array = Array::new(ctx)?;
        for (idx, item) in iter.into_iter().enumerate() {
            let item = item.into_js(ctx)?;
            array.set(idx as _, item)?;
        }
        Ok(array)
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    #[test]
    fn from_javascript() {
        test_with(|ctx| {
            let val: Array = ctx
                .eval(
                    r#"
                let a = [1,2,3,4,10,"b"]
                a[6] = {}
                a[10] = () => {"hallo"};
                a
                "#,
                )
                .unwrap();
            assert_eq!(val.len(), 11);
            assert_eq!(val.get::<i32>(3).unwrap(), 4);
            assert_eq!(val.get::<i32>(4).unwrap(), 10);
            let _six: Object = val.get(6).unwrap();
        });
    }

    #[test]
    fn into_object() {
        test_with(|ctx| {
            let val: Array = ctx
                .eval(
                    r#"
                let a = [1,2,3];
                a
            "#,
                )
                .unwrap();
            let object = val.into_object();
            assert_eq!(object.get::<_, i32>(0).unwrap(), 1);
        })
    }

    #[test]
    fn into_iter() {
        test_with(|ctx| {
            let val: Array = ctx
                .eval(
                    r#"
                      [1,'abcd',true]
                    "#,
                )
                .unwrap();
            let elems: Vec<_> = val.into_iter().collect::<Result<_>>().unwrap();
            assert_eq!(elems.len(), 3);
            assert_eq!(i8::from_js(ctx, elems[0].clone()).unwrap(), 1);
            assert_eq!(StdString::from_js(ctx, elems[1].clone()).unwrap(), "abcd");
            assert_eq!(bool::from_js(ctx, elems[2].clone()).unwrap(), true);
        })
    }

    #[test]
    fn iter() {
        test_with(|ctx| {
            let val: Array = ctx
                .eval(
                    r#"
                      ["a", 'b', '', "cdef"]
                    "#,
                )
                .unwrap();
            let elems: Vec<StdString> = val.iter().collect::<Result<_>>().unwrap();
            assert_eq!(elems.len(), 4);
            assert_eq!(elems[0], "a");
            assert_eq!(elems[1], "b");
            assert_eq!(elems[2], "");
            assert_eq!(elems[3], "cdef");
        })
    }

    #[test]
    fn collect_js() {
        test_with(|ctx| {
            let array = [1i32, 2, 3]
                .iter()
                .cloned()
                .collect_js::<Array>(ctx)
                .unwrap();
            assert_eq!(array.len(), 3);
            assert_eq!(i32::from_js(ctx, array.get(0).unwrap()).unwrap(), 1);
            assert_eq!(i32::from_js(ctx, array.get(1).unwrap()).unwrap(), 2);
            assert_eq!(i32::from_js(ctx, array.get(2).unwrap()).unwrap(), 3);
        })
    }
}
