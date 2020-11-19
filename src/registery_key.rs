use rquickjs_sys as qjs;
use std::{
    cmp::PartialEq,
    hash::{Hash, Hasher},
};

/// Key for a registery of a context.
#[derive(Clone, Copy)]
pub struct RegisteryKey(pub(crate) qjs::JSValue);

unsafe impl Send for RegisteryKey {}
unsafe impl Sync for RegisteryKey {}

impl Hash for RegisteryKey {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        unsafe { qjs::JS_VALUE_GET_PTR(self.0) }.hash(state);
        unsafe { qjs::JS_VALUE_GET_NORM_TAG(self.0) }.hash(state);
    }
}

impl PartialEq<RegisteryKey> for RegisteryKey {
    fn eq(&self, other: &Self) -> bool {
        (unsafe { qjs::JS_VALUE_GET_NORM_TAG(self.0) }
            == unsafe { qjs::JS_VALUE_GET_NORM_TAG(other.0) })
            && (unsafe { qjs::JS_VALUE_GET_PTR(self.0) }
                == unsafe { qjs::JS_VALUE_GET_PTR(other.0) })
    }
}

impl Eq for RegisteryKey {}
