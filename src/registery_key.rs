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
        unsafe { self.0.u.ptr.hash(state) };
        self.0.tag.hash(state);
    }
}

impl PartialEq<RegisteryKey> for RegisteryKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.tag == other.0.tag && unsafe { self.0.u.ptr == other.0.u.ptr }
    }
}

impl Eq for RegisteryKey {}
