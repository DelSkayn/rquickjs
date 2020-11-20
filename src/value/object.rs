use crate::{
    qjs,
    value::{self, rf::JsObjectRef},
    Atom, Ctx, FromAtom, FromIteratorJs, FromJs, IntoAtom, IntoJs, Result, Value,
};
use std::{
    iter::{IntoIterator, Iterator},
    marker::PhantomData,
    mem,
};

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
    pub fn get<K: IntoAtom<'js>, V: FromJs<'js>>(&self, k: K) -> Result<V> {
        let atom = k.into_atom(self.0.ctx);
        unsafe {
            let val = qjs::JS_GetProperty(self.0.ctx.ctx, self.0.as_js_value(), atom.atom);
            V::from_js(self.0.ctx, Value::from_js_value(self.0.ctx, val)?)
        }
    }

    /// check wether the object contains a certain key.
    pub fn contains_key<K>(&self, k: K) -> Result<bool>
    where
        K: IntoAtom<'js>,
    {
        let atom = k.into_atom(self.0.ctx);
        unsafe {
            let res = qjs::JS_HasProperty(self.0.ctx.ctx, self.0.as_js_value(), atom.atom);
            if res < 0 {
                return Err(value::get_exception(self.0.ctx));
            }
            Ok(res == 1)
        }
    }

    /// Set a member of an object to a certain value
    pub fn set<K: IntoAtom<'js>, V: IntoJs<'js>>(&self, key: K, value: V) -> Result<()> {
        let atom = key.into_atom(self.0.ctx);
        let val = value.into_js(self.0.ctx)?;
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

    /// Remove a member of an object
    pub fn remove<K: IntoAtom<'js>>(&self, key: K) -> Result<()> {
        let atom = key.into_atom(self.0.ctx);
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

    /// Get own property names of an object
    pub fn own_keys<K: FromAtom<'js>>(
        &self,
        enumerable_only: bool,
    ) -> Result<ObjectKeysIter<'js, K>> {
        let mut enums = mem::MaybeUninit::uninit();
        let mut count = mem::MaybeUninit::uninit();

        let (enums, count) = unsafe {
            let mut flags = qjs::JS_GPN_STRING_MASK as i32;
            if enumerable_only {
                flags |= qjs::JS_GPN_ENUM_ONLY as i32;
            }
            if qjs::JS_GetOwnPropertyNames(
                self.0.ctx.ctx,
                enums.as_mut_ptr(),
                count.as_mut_ptr(),
                self.0.as_js_value(),
                flags,
            ) < 0
            {
                return Err(value::get_exception(self.0.ctx));
            }
            let enums = enums.assume_init();
            let count = count.assume_init();
            (enums, count)
        };

        Ok(ObjectKeysIter {
            ctx: self.0.ctx,
            enums,
            count,
            index: 0,
            marker: PhantomData,
        })
    }

    /// Get own properties of an object
    pub fn own_props<K: FromAtom<'js>, V: FromJs<'js>>(
        &self,
        enumerable_only: bool,
    ) -> Result<ObjectIter<'js, K, V>> {
        Ok(ObjectIter {
            own_keys: Ok(self.own_keys(enumerable_only)?).into(),
            object: self.clone(),
            marker: PhantomData,
        })
    }

    /// Check if the object is a function.
    pub fn is_function(&self) -> bool {
        unsafe { qjs::JS_IsFunction(self.0.ctx.ctx, self.0.as_js_value()) != 0 }
    }

    /// Check if the object is an array.
    pub fn is_array(&self) -> bool {
        unsafe { qjs::JS_IsArray(self.0.ctx.ctx, self.0.as_js_value()) != 0 }
    }

    /// Check if the object is as error.
    pub fn is_error(&self) -> bool {
        unsafe { qjs::JS_IsError(self.0.ctx.ctx, self.0.as_js_value()) != 0 }
    }
}

/// The iterator for an object own keys
pub struct ObjectKeysIter<'js, K> {
    ctx: Ctx<'js>,
    enums: *mut qjs::JSPropertyEnum,
    index: u32,
    count: u32,
    marker: PhantomData<K>,
}

impl<'js, K> Iterator for ObjectKeysIter<'js, K>
where
    K: FromAtom<'js>,
{
    type Item = Result<K>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.count {
            let elem = unsafe { &*self.enums.offset(self.index as isize) };
            self.index += 1;
            let atom = unsafe { Atom::from_atom_val(self.ctx, elem.atom) };
            return Some(FromAtom::from_atom(atom));
        }
        None
    }
}

impl<'js, K> Drop for ObjectKeysIter<'js, K> {
    fn drop(&mut self) {
        // This is safe because iterator cannot outlife ctx
        unsafe { qjs::js_free(self.ctx.ctx, self.enums as *mut _) }
    }
}

/// The iterator for an object own properties
pub struct ObjectIter<'js, K, V> {
    own_keys: Option<Result<ObjectKeysIter<'js, Atom<'js>>>>,
    object: Object<'js>,
    marker: PhantomData<(K, V)>,
}

impl<'js, K, V> Iterator for ObjectIter<'js, K, V>
where
    K: FromAtom<'js>,
    V: FromJs<'js>,
{
    type Item = Result<(K, V)>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.own_keys {
            None => return None,
            Some(Ok(own_keys)) => {
                return match own_keys.next() {
                    Some(Ok(atom)) => Some(
                        K::from_atom(atom.clone())
                            .and_then(|key| self.object.get(atom).map(|val| (key, val))),
                    ),
                    Some(Err(error)) => Some(Err(error)),
                    _ => None,
                }
            }
            _ => (),
        }
        match self.own_keys.take() {
            Some(Err(error)) => Some(Err(error)),
            _ => unreachable!(),
        }
    }
}

impl<'js> IntoIterator for Object<'js> {
    type Item = Result<(Atom<'js>, Value<'js>)>;
    type IntoIter = ObjectIter<'js, Atom<'js>, Value<'js>>;

    fn into_iter(self) -> Self::IntoIter {
        ObjectIter {
            own_keys: self.own_keys(true).into(),
            object: self,
            marker: PhantomData,
        }
    }
}

impl<'js, K, V> FromIteratorJs<'js, (K, V)> for Object<'js>
where
    K: IntoAtom<'js>,
    V: IntoJs<'js>,
{
    type Item = (Atom<'js>, Value<'js>);

    fn from_iter_js<T>(ctx: Ctx<'js>, iter: T) -> Result<Self>
    where
        T: IntoIterator<Item = (K, V)>,
    {
        let object = Object::new(ctx)?;
        for (key, value) in iter {
            let key = key.into_atom(ctx);
            let value = value.into_js(ctx)?;
            object.set(key, value)?;
        }
        Ok(object)
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

    #[test]
    fn own_keys_iter() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let val: Object = ctx
                .eval(
                    r#"
                   ({
                     123: 123,
                     str: "abc",
                     arr: [],
                     '': undefined,
                   })
                "#,
                )
                .unwrap();
            let keys = val
                .own_keys(true)
                .unwrap()
                .collect::<Result<Vec<StdString>>>()
                .unwrap();
            assert_eq!(keys.len(), 4);
            assert_eq!(keys[0], "123");
            assert_eq!(keys[1], "str");
            assert_eq!(keys[2], "arr");
            assert_eq!(keys[3], "");
        })
    }

    #[test]
    fn own_props_iter() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let val: Object = ctx
                .eval(
                    r#"
                   ({
                     123: "",
                     str: "abc",
                     '': "def",
                   })
                "#,
                )
                .unwrap();
            let pairs = val
                .own_props(true)
                .unwrap()
                .collect::<Result<Vec<(StdString, StdString)>>>()
                .unwrap();
            assert_eq!(pairs.len(), 3);
            assert_eq!(pairs[0].0, "123");
            assert_eq!(pairs[0].1, "");
            assert_eq!(pairs[1].0, "str");
            assert_eq!(pairs[1].1, "abc");
            assert_eq!(pairs[2].0, "");
            assert_eq!(pairs[2].1, "def");
        })
    }

    #[test]
    fn into_iter() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let val: Object = ctx
                .eval(
                    r#"
                   ({
                     123: 123,
                     str: "abc",
                     arr: [],
                     '': undefined,
                   })
                "#,
                )
                .unwrap();
            let pairs = val.into_iter().collect::<Result<Vec<_>>>().unwrap();
            assert_eq!(pairs.len(), 4);
            assert_eq!(pairs[0].0.clone().to_string().unwrap(), "123");
            assert_eq!(pairs[0].1, Value::Int(123));
            assert_eq!(pairs[1].0.clone().to_string().unwrap(), "str");
            assert_eq!(StdString::from_js(ctx, pairs[1].1.clone()).unwrap(), "abc");
            assert_eq!(pairs[2].0.clone().to_string().unwrap(), "arr");
            assert_eq!(Array::from_js(ctx, pairs[2].1.clone()).unwrap().len(), 0);
            assert_eq!(pairs[3].0.clone().to_string().unwrap(), "");
            assert_eq!(pairs[3].1, Value::Undefined);
        })
    }

    #[test]
    fn collect_js() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let object = [("a", "bc"), ("$_", ""), ("", "xyz")]
                .iter()
                .cloned()
                .collect_js::<Object>(ctx)
                .unwrap();
            assert_eq!(
                StdString::from_js(ctx, object.get("a").unwrap()).unwrap(),
                "bc"
            );
            assert_eq!(
                StdString::from_js(ctx, object.get("$_").unwrap()).unwrap(),
                ""
            );
            assert_eq!(
                StdString::from_js(ctx, object.get("").unwrap()).unwrap(),
                "xyz"
            );
        })
    }
}
