use crate::{
    value::{self, rf::JsObjectRef},
    Ctx, Error, FromJs, ToJs, Value,
};
use quickjs_sys as qjs;

/// Rust representation of a javascript object.
#[derive(Debug, PartialEq, Clone)]
pub struct Object<'js>(JsObjectRef<'js>);

impl<'js> Object<'js> {
    // Unsafe because pointers must be valid and the
    // liftime of this object must be constrained
    // Further more the JSValue must also be of type object as indicated by JS_TAG_OBJECT
    // All save functions rely on this constrained to be save
    pub(crate) unsafe fn new(ctx: Ctx<'js>, v: qjs::JSValue) -> Self {
        Object(JsObjectRef::from_js_value(ctx, v))
    }

    // Save because using the JSValue is unsafe
    pub(crate) fn to_js_value(&self) -> qjs::JSValue {
        self.0.to_js_value()
    }

    pub fn get<K: ToJs<'js>, V: FromJs<'js>>(&self, k: K) -> Result<V, Error> {
        let key = k.to_js(self.0.ctx)?;
        unsafe {
            let val = match key {
                Value::Int(x) => {
                    // TODO is this correct. Integers are signed and the index here is unsigned
                    // Soo...
                    qjs::JS_GetPropertyUint32(self.0.ctx.ctx, self.to_js_value(), x as u32)
                }
                x => {
                    let atom = qjs::JS_ValueToAtom(self.0.ctx.ctx, x.to_js_value());
                    qjs::JS_GetProperty(self.0.ctx.ctx, self.to_js_value(), atom)
                }
            };
            V::from_js(self.0.ctx, Value::from_js_value(self.0.ctx, val)?)
        }
    }

    pub fn contains_key<K>(&self, k: K) -> Result<bool, Error>
    where
        K: ToJs<'js>,
    {
        let key = k.to_js(self.0.ctx)?;
        unsafe {
            let atom = qjs::JS_ValueToAtom(self.0.ctx.ctx, key.to_js_value());
            let res = qjs::JS_HasProperty(self.0.ctx.ctx, self.to_js_value(), atom);
            if res < 0 {
                return Err(value::get_exception(self.0.ctx));
            }
            Ok(res == 1)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use std::string::String as StdString;
    #[test]
    fn js_value_object_from_javascript() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let val = ctx.eval::<Value, _>(
                r#"
                let obj = {};
                obj['a'] = 3;
                obj[3] = 'a';
                obj
            "#,
            );
            if let Ok(Value::Object(x)) = val {
                let text: StdString = x.get(Value::Int(3)).unwrap();
                assert_eq!(text.as_str(), "a");
                let int: i32 = x.get("a").unwrap();
                assert_eq!(int, 3);
                let int: StdString = x.get("a").unwrap();
                assert_eq!(int, "3");
            } else {
                panic!();
            };
        });
    }
}
