use crate::{
    value::{self, rf::JsObjectRef},
    Ctx, FromJs, Object, Result, Value,
};
use rquickjs_sys as qjs;
use std::ffi::CStr;

/// Rust representation of a javascript object optimized as an array.
///
/// Javascript array's are objects and can be used as such.
/// However arrays in quickjs are optimized when they do not have any holes.
/// This value represents such a optimized array.
#[derive(Debug, PartialEq, Clone)]
pub struct Array<'js>(pub(crate) JsObjectRef<'js>);

impl<'js> Array<'js> {
    // Unsafe because pointers must be valid and the
    // liftime of this object must within the lifetime of the context
    // Further more the JSValue must also be of type object as indicated by `qjs::JS_TAG_OBJECT`.
    // It also should be a javascript array as indicated by `qjs::JS_IsArray` but this might not be
    // a hard requirement.
    // All save functions rely on this constrained to be save
    pub(crate) unsafe fn from_js_value(ctx: Ctx<'js>, v: qjs::JSValue) -> Self {
        Array(JsObjectRef::from_js_value(ctx, v))
    }

    // Save because using the JSValue is unsafe
    pub(crate) fn as_js_value(&self) -> qjs::JSValue {
        self.0.as_js_value()
    }

    pub fn new(ctx: Ctx<'js>) -> Result<Self> {
        unsafe {
            let val = qjs::JS_NewArray(ctx.ctx);
            value::handle_exception(ctx, val)?;
            Ok(Array(JsObjectRef::from_js_value(ctx, val)))
        }
    }

    pub fn to_object(self) -> Object<'js> {
        Object(self.0)
    }

    pub fn from_object(object: Object<'js>) -> Self {
        Array(object.0)
    }

    /// Get the lenght of the javascript array.
    pub fn len(&self) -> usize {
        let v = self.as_js_value();
        unsafe {
            let prop = CStr::from_bytes_with_nul(b"length\0").unwrap();
            let val = qjs::JS_GetPropertyStr(self.0.ctx.ctx, v, prop.as_ptr());
            assert!(qjs::JS_IsInt(val));
            qjs::JS_VALUE_GET_INT!(val) as usize
        }
    }

    /// Returns wether a javascript array is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the value at a index in the javascript array.
    pub fn get<V: FromJs<'js>>(&self, idx: u32) -> Result<V> {
        unsafe {
            let v = self.as_js_value();
            let val = qjs::JS_GetPropertyUint32(self.0.ctx.ctx, v, idx);
            let val = Value::from_js_value(self.0.ctx, val)?;
            V::from_js(self.0.ctx, val)
        }
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
                assert_eq!(x.get(3), Ok(4));
                assert_eq!(x.get(4), Ok(10));
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
    fn to_object() {
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
            let object = val.to_object();
            assert_eq!(object.get(0), Ok(1));
        })
    }
}
