use crate::Error;
use rquickjs_sys as qjs;
use std::{
    cell::RefCell,
    ffi::CString,
    ptr,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub(crate) struct Inner {
    //TODO: Maybe make this NonNull?
    pub(crate) rt: *mut qjs::JSRuntime,
    pub(crate) lock: Mutex<()>,
    // Keep info alive for the entire duration of the lifetime of rt
    info: RefCell<Option<CString>>,
}

/// Entry point of the library.
#[derive(Debug)]
pub struct Runtime {
    pub(crate) inner: Arc<Inner>,
}

impl Runtime {
    pub fn new() -> Result<Self, Error> {
        let rt = unsafe { qjs::JS_NewRuntime() };
        if rt == ptr::null_mut() {
            return Err(Error::Allocation);
        }
        Ok(Runtime {
            inner: Arc::new(Inner {
                rt,
                info: RefCell::new(None),
                lock: Mutex::new(()),
            }),
        })
    }

    pub fn set_info<S: Into<Vec<u8>>>(&mut self, info: S) -> Result<(), Error> {
        let string = CString::new(info)?;
        unsafe { qjs::JS_SetRuntimeInfo(self.inner.rt, string.as_ptr()) }
        *self.inner.info.borrow_mut() = Some(string);
        Ok(())
    }

    pub fn set_memory_limit(&self, limit: usize) {
        let limit = limit as qjs::size_t;
        unsafe { qjs::JS_SetMemoryLimit(self.inner.rt, limit) }
    }

    pub fn set_gc_threshold(&self, threshold: usize) {
        let threshold = threshold as qjs::size_t;
        unsafe { qjs::JS_SetGCThreshold(self.inner.rt, threshold) }
    }

    pub fn run_gc(&self) {
        unsafe { qjs::JS_RunGC(self.inner.rt) }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        unsafe { qjs::JS_FreeRuntime(self.rt) }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn base_runtime() {
        let mut rt = Runtime::new().unwrap();
        rt.set_info("test runtime").unwrap();
        rt.set_memory_limit(0xFFFF);
        rt.set_gc_threshold(0xFF);
        rt.run_gc();
    }
}
