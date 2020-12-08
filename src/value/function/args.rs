use crate::{qjs, Ctx, Value};
use std::iter::{ExactSizeIterator, FusedIterator};

/// An iterator over a list of js values
pub struct ArgsIter<'js> {
    ctx: Ctx<'js>,
    ptr: *mut qjs::JSValue,
    count: usize,
    index: usize,
}

impl<'js> ArgsIter<'js> {
    pub(crate) unsafe fn from_value_count_const(
        ctx: Ctx<'js>,
        count: usize,
        ptr: *mut qjs::JSValue,
    ) -> Self {
        Self {
            ctx,
            ptr,
            count,
            index: 0,
        }
    }
}

impl<'js> Iterator for ArgsIter<'js> {
    type Item = Value<'js>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.count {
            return None;
        }
        unsafe {
            let ptr = self.ptr.add(self.index);
            self.index += 1;
            Some(Value::from_js_value_const(self.ctx, *ptr))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.len();
        (size, Some(size))
    }
}

impl<'js> FusedIterator for ArgsIter<'js> {}

impl<'js> ExactSizeIterator for ArgsIter<'js> {
    fn len(&self) -> usize {
        self.count - self.index
    }
}
