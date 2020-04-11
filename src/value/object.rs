use crate::{
    value::{self, rf::JsObjectRef},
    Ctx, FromJs, Result, ToAtom, ToJs, Value,
};
use rquickjs_sys as qjs;

/// Rust representation of a javascript object.
#[derive(Debug, PartialEq, Clone)]
pub struct Object<'js>(pub(crate) JsObjectRef<'js>);

impl<'js> Object<'js> {
    /// Create a new javascript object
    pub fn new(ctx: Ctx<'js>) -> Result<Self> {
        unsafe {
            let val = qjs::JS_NewObject(ctx.ctx);
            let val = value::handle_exception(ctx, val)?;
            Ok(Object(JsObjectRef::from_js_value(ctx, val)))
        }
    }

    /// Get a new value
    pub fn get<K: ToAtom<'js>, V: FromJs<'js>>(&self, k: K) -> Result<V> {
        let atom = k.to_atom(self.0.ctx);
        unsafe {
            let val = qjs::JS_GetProperty(self.0.ctx.ctx, self.0.as_js_value(), atom.atom);
            V::from_js(self.0.ctx, Value::from_js_value(self.0.ctx, val)?)
        }
    }

    /// check wether the object contains a certain key.
    pub fn contains_key<K>(&self, k: K) -> Result<bool>
    where
        K: ToAtom<'js>,
    {
        let atom = k.to_atom(self.0.ctx);
        unsafe {
            let res = qjs::JS_HasProperty(self.0.ctx.ctx, self.0.as_js_value(), atom.atom);
            if res < 0 {
                return Err(value::get_exception(self.0.ctx));
            }
            Ok(res == 1)
        }
    }

    /// Set a member of an object to a certain value
    pub fn set<K: ToAtom<'js>, V: ToJs<'js>>(&self, key: K, value: V) -> Result<()> {
        let atom = key.to_atom(self.0.ctx);
        let val = value.to_js(self.0.ctx)?;
        unsafe {
            if qjs::JS_SetProperty(
                self.0.ctx.ctx,
                self.0.as_js_value(),
                atom.atom,
                val.into_js_value(),
            ) < 0
            {
                return Err(value::get_exception(self.0.ctx));
            }
        }
        Ok(())
    }

    /// Remove a member of this objects
    pub fn remove<K: ToAtom<'js>>(&self, key: K) -> Result<()> {
        let atom = key.to_atom(self.0.ctx);
        unsafe {
            if qjs::JS_DeleteProperty(
                self.0.ctx.ctx,
                self.0.as_js_value(),
                atom.atom,
                qjs::JS_PROP_THROW as i32,
            ) < 0
            {
                return Err(value::get_exception(self.0.ctx));
            }
        }
        Ok(())
    }

    /// Check if the object is a function.
    pub fn is_function(&self) -> bool {
        unsafe { qjs::JS_IsFunction(self.0.ctx.ctx, self.0.as_js_value()) != 0 }
    }

    /// Check if the object is an array.
    pub fn is_array(&self) -> bool {
        unsafe { qjs::JS_IsArray(self.0.ctx.ctx, self.0.as_js_value()) != 0 }
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use std::string::String as StdString;
    #[test]
    fn from_javascript() {
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
                let int: StdString = x.get(3).unwrap();
                assert_eq!(int, "a");
                x.set("hallo", "foo").unwrap();
                assert_eq!(x.get::<_, StdString>("hallo").unwrap(), "foo".to_string());
                x.remove("hallo").unwrap();
                assert_eq!(x.get::<_, Value>("hallo").unwrap(), Value::Undefined)
            } else {
                panic!();
            };
        });
    }

    #[test]
    fn types() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let val: Object = ctx
                .eval(
                    r#"
                let array_3 = [];
                array_3[3] = "foo";
                array_3[99] = 4;
                ({
                    array_1: [0,1,2,3,4,5],
                    array_2: [0,"foo",{},undefined,4,5],
                    array_3: array_3,
                    func_1: () => 1,
                    func_2: function(){ return "foo"},
                    obj_1: {
                        a: 1,
                        b: "foo",
                    },
                })
                "#,
                )
                .unwrap();
            assert!(val.get::<_, Object>("array_1").unwrap().is_array());
            assert!(val.get::<_, Object>("array_2").unwrap().is_array());
            assert!(val.get::<_, Object>("array_3").unwrap().is_array());
            assert!(val.get::<_, Object>("func_1").unwrap().is_function());
            assert!(val.get::<_, Object>("func_2").unwrap().is_function());
            assert!(!val.get::<_, Object>("obj_1").unwrap().is_function());
            assert!(!val.get::<_, Object>("obj_1").unwrap().is_array());
        })
    }
}
