use crate::{qjs, Ctx, Value};
use std::{
    iter::{ExactSizeIterator, FusedIterator},
    mem,
    ops::{Deref, DerefMut},
};

/// An iterator over a list of js values
pub struct ValueIter<'js> {
    value: mem::ManuallyDrop<ArgsValue<'js>>,
    current: usize,
}

impl<'js> Iterator for ValueIter<'js> {
    type Item = Value<'js>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.value.len == self.current {
            return None;
        }
        unsafe {
            let ptr = self.value.ptr.add(self.current);
            self.current += 1;
            if self.value.ownership {
                Some(Value::from_js_value(self.value.ctx, *ptr).unwrap())
            } else {
                Some(Value::from_js_value_const(self.value.ctx, *ptr).unwrap())
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.value.len, Some(self.value.len))
    }
}

impl<'js> FusedIterator for ValueIter<'js> {}

impl<'js> ExactSizeIterator for ValueIter<'js> {
    fn len(&self) -> usize {
        self.value.len - self.current
    }
}

impl<'js> Drop for ValueIter<'js> {
    fn drop(&mut self) {
        unsafe {
            if self.value.ownership {
                self.current += 1;
                for v in self.current..self.value.len {
                    let ptr = self.value.ptr.add(v);
                    Value::from_js_value(self.value.ctx, *ptr).ok();
                }
            }
        }
    }
}

/// An list of values given from JS
///
/// Handed to callbacks as arguments.
pub struct ArgsValue<'js> {
    ctx: Ctx<'js>,
    len: usize,
    ptr: *mut qjs::JSValue,
    ownership: bool,
}

impl<'js> Clone for ArgsValue<'js> {
    fn clone(&self) -> Self {
        ArgsValue {
            ctx: self.ctx,
            len: self.len,
            ptr: self.ptr,
            ownership: false,
        }
    }
}

impl<'js> ArgsValue<'js> {
    #[allow(dead_code)]
    pub(crate) unsafe fn from_value_count(
        ctx: Ctx<'js>,
        len: usize,
        ptr: *mut qjs::JSValue,
    ) -> Self {
        ArgsValue {
            ctx,
            len,
            ptr,
            ownership: true,
        }
    }

    #[allow(dead_code)]
    pub(crate) unsafe fn from_value_count_const(
        ctx: Ctx<'js>,
        len: usize,
        ptr: *mut qjs::JSValue,
    ) -> Self {
        ArgsValue {
            ctx,
            len,
            ptr,
            ownership: false,
        }
    }

    /// Returns the number of js values this multi value contains.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns wether there are no js values in multi value container.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns a vector containing all the js values.
    pub fn into_vec(mut self) -> Vec<Value<'js>> {
        self.iter().collect()
    }

    /// Returns a interator over the js values.
    pub fn iter(&mut self) -> ValueIter<'js> {
        let res = ValueIter {
            value: mem::ManuallyDrop::new(ArgsValue {
                ctx: self.ctx,
                len: self.len,
                ptr: self.ptr,
                ownership: self.ownership,
            }),
            current: 0,
        };
        self.ownership = false;
        res
    }
}

impl<'js> Drop for ArgsValue<'js> {
    fn drop(&mut self) {
        mem::drop(self.iter())
    }
}

/// An list of values to pass to JS
///
/// Passed to functions as arguments when calling.
#[derive(Clone, Default)]
pub struct ArgsValueJs<'js>(Vec<Value<'js>>);

impl<'js> ArgsValueJs<'js> {
    pub fn new() -> Self {
        Self(Vec::new())
    }
}

impl<'js> From<Vec<Value<'js>>> for ArgsValueJs<'js> {
    fn from(vec: Vec<Value<'js>>) -> Self {
        Self(vec)
    }
}

impl<'js> Into<Vec<Value<'js>>> for ArgsValueJs<'js> {
    fn into(self) -> Vec<Value<'js>> {
        self.0
    }
}

impl<'js> Deref for ArgsValueJs<'js> {
    type Target = Vec<Value<'js>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'js> DerefMut for ArgsValueJs<'js> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Rest values
#[derive(Clone, Default)]
pub struct RestValues<T>(Vec<T>);

impl<T> RestValues<T> {
    pub fn new() -> Self {
        Self(Vec::new())
    }
}

impl<T> From<Vec<T>> for RestValues<T> {
    fn from(vec: Vec<T>) -> Self {
        Self(vec)
    }
}

impl<T> Into<Vec<T>> for RestValues<T> {
    fn into(self) -> Vec<T> {
        self.0
    }
}

impl<T> Deref for RestValues<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for RestValues<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
