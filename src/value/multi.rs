use crate::{Ctx, Value};
use rquickjs_sys as qjs;
use std::{
    iter::{ExactSizeIterator, FusedIterator},
    mem,
};

/// An iterator over a list of js values
pub struct ValueIter<'js> {
    value: mem::ManuallyDrop<MultiValue<'js>>,
    current: usize,
}

impl<'js> Iterator for ValueIter<'js> {
    type Item = Value<'js>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.value.len == self.current {
            return None;
        }
        unsafe {
            let ptr = self.value.ptr.offset(self.current as isize);
            self.current += 1;
            if self.value.ownership {
                return Some(Value::from_js_value(self.value.ctx, *ptr).unwrap());
            } else {
                return Some(Value::from_js_value_const(self.value.ctx, *ptr).unwrap());
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
                    let ptr = self.value.ptr.offset(v as isize);
                    Value::from_js_value(self.value.ctx, *ptr).ok();
                }
            }
        }
    }
}

/// An list of Js values.
///
/// Handed to callbacks as arguments.
pub struct MultiValue<'js> {
    ctx: Ctx<'js>,
    len: usize,
    ptr: *mut qjs::JSValue,
    ownership: bool,
}

impl<'js> Clone for MultiValue<'js> {
    fn clone(&self) -> Self {
        MultiValue {
            ctx: self.ctx,
            len: self.len,
            ptr: self.ptr,
            ownership: false,
        }
    }
}

impl<'js> MultiValue<'js> {
    #[allow(dead_code)]
    pub(crate) unsafe fn from_value_count(
        ctx: Ctx<'js>,
        len: usize,
        ptr: *mut qjs::JSValue,
    ) -> Self {
        MultiValue {
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
        MultiValue {
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
    pub fn to_vec(mut self) -> Vec<Value<'js>> {
        self.iter().collect()
    }

    /// Returns a interator over the js values.
    pub fn iter(&mut self) -> ValueIter<'js> {
        let res = ValueIter {
            value: mem::ManuallyDrop::new(MultiValue {
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

impl<'js> Drop for MultiValue<'js> {
    fn drop(&mut self) {
        mem::drop(self.iter())
    }
}
