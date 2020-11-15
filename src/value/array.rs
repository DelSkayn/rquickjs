use crate::{
    value::{self, rf::JsObjectRef},
    Ctx, FromIteratorJs, FromJs, Object, Result, ToJs, Value,
};
use rquickjs_sys as qjs;
use std::{
    iter::{IntoIterator, Iterator},
    marker::PhantomData,
};

/// Rust representation of a javascript object optimized as an array.
///
/// Javascript array's are objects and can be used as such.
/// However arrays in quickjs are optimized when they do not have any holes.
/// This value represents such a optimized array.
#[derive(Debug, PartialEq, Clone)]
pub struct Array<'js>(pub(crate) JsObjectRef<'js>);

impl<'js> Array<'js> {
    pub fn new(ctx: Ctx<'js>) -> Result<Self> {
        unsafe {
            let val = qjs::JS_NewArray(ctx.ctx);
            value::handle_exception(ctx, val)?;
            Ok(Array(JsObjectRef::from_js_value(ctx, val)))
        }
    }

    pub fn into_object(self) -> Object<'js> {
        Object(self.0)
    }

    pub fn from_object(object: Object<'js>) -> Self {
        Array(object.0)
    }

    /// Get the lenght of the javascript array.
    pub fn len(&self) -> usize {
        let v = self.0.as_js_value();
        unsafe {
            let val = qjs::JS_GetPropertyStr(self.0.ctx.ctx, v, b"length\0".as_ptr() as *const _);
            assert!(qjs::JS_IsInt(val));
            qjs::JS_VALUE_GET_INT!(val) as usize
        }
    }

    /// Returns wether a javascript array is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the value at an index in the javascript array.
    pub fn get<V: FromJs<'js>>(&self, idx: u32) -> Result<V> {
        let obj = self.0.as_js_value();
        let val = unsafe {
            let val = qjs::JS_GetPropertyUint32(self.0.ctx.ctx, obj, idx);
            Value::from_js_value(self.0.ctx, val)
        }?;
        V::from_js(self.0.ctx, val)
    }

    /// Set the value at an index in the javascript array.
    pub fn set<V: ToJs<'js>>(&self, idx: u32, val: V) -> Result<()> {
        let obj = self.0.as_js_value();
        let val = val.to_js(self.0.ctx)?.into_js_value();
        unsafe {
            if -1 == qjs::JS_SetPropertyUint32(self.0.ctx.ctx, obj, idx, val) {
                return Err(value::get_exception(self.0.ctx));
            }
        }
        Ok(())
    }

    /// Get iterator over elments of an array
    pub fn iter<T: FromJs<'js>>(&self) -> ArrayIter<'js, T> {
        let count = self.len() as u32;
        ArrayIter {
            array: self.clone(),
            index: 0,
            count,
            marker: PhantomData,
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
            let res = self.array.get(self.index);
            self.index += 1;
            Some(res)
        } else {
            None
        }
    }
}

impl<'js> IntoIterator for Array<'js> {
    type Item = Result<Value<'js>>;
    type IntoIter = ArrayIter<'js, Value<'js>>;

    fn into_iter(self) -> Self::IntoIter {
        let count = self.len() as u32;
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
    A: ToJs<'js>,
{
    type Item = Value<'js>;

    fn from_iter_js<T>(ctx: Ctx<'js>, iter: T) -> Result<Self>
    where
        T: IntoIterator<Item = A>,
    {
        let array = Array::new(ctx)?;
        let mut index = 0;
        for item in iter {
            let item = item.to_js(ctx)?;
            array.set(index, item)?;
            index += 1;
        }
        Ok(array)
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    #[test]
    fn from_javascript() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let val = ctx.eval::<Value, _>(
                r#"
                let a = [1,2,3,4,10,"b"]
                a[6] = {}
                a[10] = () => {"hallo"};
                a
                "#,
            );
            if let Ok(Value::Array(x)) = val {
                assert_eq!(x.len(), 11);
                assert_eq!(x.get::<i32>(3).unwrap(), 4);
                assert_eq!(x.get::<i32>(4).unwrap(), 10);
                if let Ok(Value::Object(_)) = x.get(6) {
                } else {
                    panic!();
                }
            } else {
                panic!();
            };
        });
    }

    #[test]
    fn into_object() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let val = ctx
                .eval::<Array, _>(
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
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let val: Array = ctx
                .eval(
                    r#"
                      [1,'abcd',true]
                    "#,
                )
                .unwrap();
            let elems: Vec<_> = val.into_iter().collect::<Result<_>>().unwrap();
            assert_eq!(elems.len(), 3);
            assert_eq!(elems[0], Value::Int(1));
            assert_eq!(StdString::from_js(ctx, elems[1].clone()).unwrap(), "abcd");
            assert_eq!(elems[2], Value::Bool(true));
        })
    }

    #[test]
    fn iter() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
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
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
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
