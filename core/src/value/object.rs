//! Module for types dealing with JS objects.

use crate::{
    convert::FromIteratorJs, qjs, Array, Atom, Ctx, Error, FromAtom, FromJs, IntoAtom, IntoJs,
    Result, Value,
};
use std::{
    iter::{DoubleEndedIterator, ExactSizeIterator, FusedIterator, IntoIterator, Iterator},
    marker::PhantomData,
    mem,
};
mod property;
pub use property::{Accessor, AsProperty, Property, PropertyFlags};

/// Rust representation of a javascript object.
#[derive(Debug, PartialEq, Clone)]
#[repr(transparent)]
pub struct Object<'js>(pub(crate) Value<'js>);

impl<'js> Object<'js> {
    /// Create a new javascript object
    pub fn new(ctx: Ctx<'js>) -> Result<Self> {
        Ok(unsafe {
            let val = qjs::JS_NewObject(ctx.as_ptr());
            let val = ctx.handle_exception(val)?;
            Object::from_js_value(ctx, val)
        })
    }

    /// Get a new value
    pub fn get<K: IntoAtom<'js>, V: FromJs<'js>>(&self, k: K) -> Result<V> {
        let atom = k.into_atom(self.0.ctx)?;
        V::from_js(self.0.ctx, unsafe {
            let val = qjs::JS_GetProperty(self.0.ctx.as_ptr(), self.0.as_js_value(), atom.atom);
            let val = self.0.ctx.handle_exception(val)?;
            Value::from_js_value(self.0.ctx, val)
        })
    }

    /// check wether the object contains a certain key.
    pub fn contains_key<K>(&self, k: K) -> Result<bool>
    where
        K: IntoAtom<'js>,
    {
        let atom = k.into_atom(self.0.ctx)?;
        unsafe {
            let res = qjs::JS_HasProperty(self.0.ctx.as_ptr(), self.0.as_js_value(), atom.atom);
            if res < 0 {
                return Err(self.0.ctx.raise_exception());
            }
            Ok(res == 1)
        }
    }

    /// Set a member of an object to a certain value
    pub fn set<K: IntoAtom<'js>, V: IntoJs<'js>>(&self, key: K, value: V) -> Result<()> {
        let atom = key.into_atom(self.0.ctx)?;
        let val = value.into_js(self.0.ctx)?;
        unsafe {
            if qjs::JS_SetProperty(
                self.0.ctx.as_ptr(),
                self.0.as_js_value(),
                atom.atom,
                val.into_js_value(),
            ) < 0
            {
                return Err(self.0.ctx.raise_exception());
            }
        }
        Ok(())
    }

    /// Remove a member of an object
    pub fn remove<K: IntoAtom<'js>>(&self, key: K) -> Result<()> {
        let atom = key.into_atom(self.0.ctx)?;
        unsafe {
            if qjs::JS_DeleteProperty(
                self.0.ctx.as_ptr(),
                self.0.as_js_value(),
                atom.atom,
                qjs::JS_PROP_THROW as _,
            ) < 0
            {
                return Err(self.0.ctx.raise_exception());
            }
        }
        Ok(())
    }

    /// Check the object for empty
    pub fn is_empty(&self) -> bool {
        self.keys::<Atom>().next().is_none()
    }

    /// Get the number of properties
    pub fn len(&self) -> usize {
        self.keys::<Atom>().count()
    }

    /// Get own string enumerable property names of an object
    pub fn keys<K: FromAtom<'js>>(&self) -> ObjectKeysIter<'js, K> {
        self.own_keys(Filter::default())
    }

    /// Get own property names of an object
    pub fn own_keys<K: FromAtom<'js>>(&self, filter: Filter) -> ObjectKeysIter<'js, K> {
        ObjectKeysIter {
            state: Some(IterState::new(&self.0, filter.flags)),
            marker: PhantomData,
        }
    }

    /// Get own string enumerable properties of an object
    pub fn props<K: FromAtom<'js>, V: FromJs<'js>>(&self) -> ObjectIter<'js, K, V> {
        self.own_props(Filter::default())
    }

    /// Get own properties of an object
    pub fn own_props<K: FromAtom<'js>, V: FromJs<'js>>(
        &self,
        filter: Filter,
    ) -> ObjectIter<'js, K, V> {
        ObjectIter {
            state: Some(IterState::new(&self.0, filter.flags)),
            object: self.clone(),
            marker: PhantomData,
        }
    }

    /// Get own string enumerable property values of an object
    pub fn values<K: FromAtom<'js>>(&self) -> ObjectValuesIter<'js, K> {
        self.own_values(Filter::default())
    }

    /// Get own property values of an object
    pub fn own_values<K: FromAtom<'js>>(&self, filter: Filter) -> ObjectValuesIter<'js, K> {
        ObjectValuesIter {
            state: Some(IterState::new(&self.0, filter.flags)),
            object: self.clone(),
            marker: PhantomData,
        }
    }

    /// Get an object prototype
    ///
    /// Objects can have no prototype, in this case this function will return null.
    pub fn get_prototype(&self) -> Option<Object<'js>> {
        unsafe {
            let proto = qjs::JS_GetPrototype(self.0.ctx.as_ptr(), self.0.as_js_value());
            if qjs::JS_IsNull(proto) {
                None
            } else {
                Some(Object::from_js_value(self.0.ctx, proto))
            }
        }
    }

    /// Set an object prototype
    ///
    /// If called with None the function will set the prototype of the object to null.
    ///
    /// This function will error if setting the prototype causes a cycle in the prototype chain.
    pub fn set_prototype(&self, proto: Option<&Object<'js>>) -> Result<()> {
        let proto = proto.map(|x| x.as_js_value()).unwrap_or(qjs::JS_NULL);
        unsafe {
            if 1 != qjs::JS_SetPrototype(self.0.ctx.as_ptr(), self.0.as_js_value(), proto) {
                Err(self.0.ctx.raise_exception())
            } else {
                Ok(())
            }
        }
    }

    /// Check instance of object
    pub fn is_instance_of(&self, class: impl AsRef<Value<'js>>) -> bool {
        let class = class.as_ref();
        0 != unsafe {
            qjs::JS_IsInstanceOf(
                self.0.ctx.as_ptr(),
                self.0.as_js_value(),
                class.as_js_value(),
            )
        }
    }

    /// Convert into an array
    pub fn into_array(self) -> Option<Array<'js>> {
        if self.is_array() {
            Some(Array(self))
        } else {
            None
        }
    }
}

/// The property filter
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Filter {
    flags: qjs::c_int,
}

/// Include only enumerable string properties by default
impl Default for Filter {
    fn default() -> Self {
        Self::new().string().enum_only()
    }
}

impl Filter {
    /// Create filter which includes nothing
    pub fn new() -> Self {
        Self { flags: 0 }
    }

    /// Include string properties
    #[must_use]
    pub fn string(mut self) -> Self {
        self.flags |= qjs::JS_GPN_STRING_MASK as qjs::c_int;
        self
    }

    /// Include symbol properties
    #[must_use]
    pub fn symbol(mut self) -> Self {
        self.flags |= qjs::JS_GPN_SYMBOL_MASK as qjs::c_int;
        self
    }

    /// Include private properties
    #[must_use]
    pub fn private(mut self) -> Self {
        self.flags |= qjs::JS_GPN_PRIVATE_MASK as qjs::c_int;
        self
    }

    /// Include only enumerable properties
    #[must_use]
    pub fn enum_only(mut self) -> Self {
        self.flags |= qjs::JS_GPN_ENUM_ONLY as qjs::c_int;
        self
    }
}

struct IterState<'js> {
    ctx: Ctx<'js>,
    enums: *mut qjs::JSPropertyEnum,
    index: u32,
    count: u32,
}

impl<'js> IterState<'js> {
    fn new(obj: &Value<'js>, flags: qjs::c_int) -> Result<Self> {
        let ctx = obj.ctx;

        let mut enums = mem::MaybeUninit::uninit();
        let mut count = mem::MaybeUninit::uninit();

        let (enums, count) = unsafe {
            if qjs::JS_GetOwnPropertyNames(
                ctx.as_ptr(),
                enums.as_mut_ptr(),
                count.as_mut_ptr(),
                obj.value,
                flags,
            ) < 0
            {
                return Err(ctx.raise_exception());
            }
            let enums = enums.assume_init();
            let count = count.assume_init();
            (enums, count)
        };

        Ok(Self {
            ctx,
            enums,
            count,
            index: 0,
        })
    }
}

impl<'js> Drop for IterState<'js> {
    fn drop(&mut self) {
        // Free atoms which doesn't consumed by the iterator
        for index in self.index..self.count {
            let elem = unsafe { &*self.enums.offset(index as isize) };
            unsafe { qjs::JS_FreeAtom(self.ctx.as_ptr(), elem.atom) };
        }

        // This is safe because iterator cannot outlive ctx
        unsafe { qjs::js_free(self.ctx.as_ptr(), self.enums as _) };
    }
}

impl<'js> Iterator for IterState<'js> {
    type Item = Atom<'js>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.count {
            let elem = unsafe { &*self.enums.offset(self.index as _) };
            self.index += 1;
            let atom = unsafe { Atom::from_atom_val(self.ctx, elem.atom) };
            Some(atom)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'js> DoubleEndedIterator for IterState<'js> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.index < self.count {
            self.count -= 1;
            let elem = unsafe { &*self.enums.offset(self.count as _) };
            let atom = unsafe { Atom::from_atom_val(self.ctx, elem.atom) };
            Some(atom)
        } else {
            None
        }
    }
}

impl<'js> ExactSizeIterator for IterState<'js> {
    fn len(&self) -> usize {
        (self.count - self.index) as _
    }
}

impl<'js> FusedIterator for IterState<'js> {}

/// The iterator for an object own keys
pub struct ObjectKeysIter<'js, K> {
    state: Option<Result<IterState<'js>>>,
    marker: PhantomData<K>,
}

impl<'js, K> Iterator for ObjectKeysIter<'js, K>
where
    K: FromAtom<'js>,
{
    type Item = Result<K>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(Ok(state)) = &mut self.state {
            match state.next() {
                Some(atom) => Some(K::from_atom(atom)),
                None => {
                    self.state = None;
                    None
                }
            }
        } else if self.state.is_none() {
            None
        } else if let Some(Err(error)) = self.state.take() {
            Some(Err(error))
        } else {
            unreachable!();
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'js, K> DoubleEndedIterator for ObjectKeysIter<'js, K>
where
    K: FromAtom<'js>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(Ok(state)) = &mut self.state {
            match state.next_back() {
                Some(atom) => Some(K::from_atom(atom)),
                None => {
                    self.state = None;
                    None
                }
            }
        } else if self.state.is_none() {
            None
        } else if let Some(Err(error)) = self.state.take() {
            Some(Err(error))
        } else {
            unreachable!();
        }
    }
}

impl<'js, K> ExactSizeIterator for ObjectKeysIter<'js, K>
where
    K: FromAtom<'js>,
{
    fn len(&self) -> usize {
        if let Some(Ok(state)) = &self.state {
            state.len()
        } else {
            0
        }
    }
}

impl<'js, K> FusedIterator for ObjectKeysIter<'js, K> where K: FromAtom<'js> {}

/// The iterator for an object own properties
pub struct ObjectIter<'js, K, V> {
    state: Option<Result<IterState<'js>>>,
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
        if let Some(Ok(state)) = &mut self.state {
            match state.next() {
                Some(atom) => Some(
                    K::from_atom(atom.clone())
                        .and_then(|key| self.object.get(atom).map(|val| (key, val))),
                ),
                None => {
                    self.state = None;
                    None
                }
            }
        } else if self.state.is_none() {
            None
        } else if let Some(Err(error)) = self.state.take() {
            Some(Err(error))
        } else {
            unreachable!();
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'js, K, V> DoubleEndedIterator for ObjectIter<'js, K, V>
where
    K: FromAtom<'js>,
    V: FromJs<'js>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(Ok(state)) = &mut self.state {
            match state.next_back() {
                Some(atom) => Some(
                    K::from_atom(atom.clone())
                        .and_then(|key| self.object.get(atom).map(|val| (key, val))),
                ),
                None => {
                    self.state = None;
                    None
                }
            }
        } else if self.state.is_none() {
            None
        } else if let Some(Err(error)) = self.state.take() {
            Some(Err(error))
        } else {
            unreachable!();
        }
    }
}

impl<'js, K, V> ExactSizeIterator for ObjectIter<'js, K, V>
where
    K: FromAtom<'js>,
    V: FromJs<'js>,
{
    fn len(&self) -> usize {
        if let Some(Ok(state)) = &self.state {
            state.len()
        } else {
            0
        }
    }
}

impl<'js, K, V> FusedIterator for ObjectIter<'js, K, V>
where
    K: FromAtom<'js>,
    V: FromJs<'js>,
{
}

/// The iterator for an object own property values
pub struct ObjectValuesIter<'js, V> {
    state: Option<Result<IterState<'js>>>,
    object: Object<'js>,
    marker: PhantomData<V>,
}

impl<'js, V> Iterator for ObjectValuesIter<'js, V>
where
    V: FromJs<'js>,
{
    type Item = Result<V>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(Ok(state)) = &mut self.state {
            match state.next() {
                Some(atom) => Some(self.object.get(atom)),
                None => {
                    self.state = None;
                    None
                }
            }
        } else if self.state.is_none() {
            None
        } else if let Some(Err(error)) = self.state.take() {
            Some(Err(error))
        } else {
            unreachable!();
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'js, V> DoubleEndedIterator for ObjectValuesIter<'js, V>
where
    V: FromJs<'js>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(Ok(state)) = &mut self.state {
            match state.next_back() {
                Some(atom) => Some(self.object.get(atom)),
                None => {
                    self.state = None;
                    None
                }
            }
        } else if self.state.is_none() {
            None
        } else if let Some(Err(error)) = self.state.take() {
            Some(Err(error))
        } else {
            unreachable!();
        }
    }
}

impl<'js, V> ExactSizeIterator for ObjectValuesIter<'js, V>
where
    V: FromJs<'js>,
{
    fn len(&self) -> usize {
        if let Some(Ok(state)) = &self.state {
            state.len()
        } else {
            0
        }
    }
}

impl<'js, V> FusedIterator for ObjectValuesIter<'js, V> where V: FromJs<'js> {}

impl<'js> IntoIterator for Object<'js> {
    type Item = Result<(Atom<'js>, Value<'js>)>;
    type IntoIter = ObjectIter<'js, Atom<'js>, Value<'js>>;

    fn into_iter(self) -> Self::IntoIter {
        let flags = qjs::JS_GPN_STRING_MASK as _;
        ObjectIter {
            state: Some(IterState::new(&self.0, flags)),
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
            let key = key.into_atom(ctx)?;
            let value = value.into_js(ctx)?;
            object.set(key, value)?;
        }
        Ok(object)
    }
}

#[cfg(test)]
mod test {
    use crate::{prelude::*, *};

    #[test]
    fn from_javascript() {
        test_with(|ctx| {
            let val: Object = ctx
                .eval(
                    r#"
                let obj = {};
                obj['a'] = 3;
                obj[3] = 'a';
                obj
            "#,
                )
                .unwrap();

            let text: StdString = val.get(3).unwrap();
            assert_eq!(text, "a");
            let int: i32 = val.get("a").unwrap();
            assert_eq!(int, 3);
            let int: StdString = val.get(3).unwrap();
            assert_eq!(int, "a");
            val.set("hallo", "foo").unwrap();
            let text: StdString = val.get("hallo").unwrap();
            assert_eq!(text, "foo".to_string());
            val.remove("hallo").unwrap();
            let text: Option<StdString> = val.get("hallo").unwrap();
            assert_eq!(text, None);
        });
    }

    #[test]
    fn types() {
        test_with(|ctx| {
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
        test_with(|ctx| {
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
            let keys = val.keys().collect::<Result<Vec<StdString>>>().unwrap();
            assert_eq!(keys.len(), 4);
            assert_eq!(keys[0], "123");
            assert_eq!(keys[1], "str");
            assert_eq!(keys[2], "arr");
            assert_eq!(keys[3], "");
        })
    }

    #[test]
    fn own_props_iter() {
        test_with(|ctx| {
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
                .props()
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
        test_with(|ctx| {
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
            assert_eq!(i32::from_js(ctx, pairs[0].1.clone()).unwrap(), 123);
            assert_eq!(pairs[1].0.clone().to_string().unwrap(), "str");
            assert_eq!(StdString::from_js(ctx, pairs[1].1.clone()).unwrap(), "abc");
            assert_eq!(pairs[2].0.clone().to_string().unwrap(), "arr");
            assert_eq!(Array::from_js(ctx, pairs[2].1.clone()).unwrap().len(), 0);
            assert_eq!(pairs[3].0.clone().to_string().unwrap(), "");
            assert_eq!(
                Undefined::from_js(ctx, pairs[3].1.clone()).unwrap(),
                Undefined
            );
        })
    }

    #[test]
    fn iter_take() {
        test_with(|ctx| {
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
                .keys()
                .take(1)
                .collect::<Result<Vec<StdString>>>()
                .unwrap();
            assert_eq!(keys.len(), 1);
            assert_eq!(keys[0], "123");
        })
    }

    #[test]
    fn collect_js() {
        test_with(|ctx| {
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
