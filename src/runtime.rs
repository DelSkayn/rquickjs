use crate::Error;
use rquickjs_sys as qjs;
use std::{
    ffi::CString,
    mem, ptr,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub(crate) struct Inner {
    //TODO: Maybe make this NonNull?
    pub(crate) rt: *mut qjs::JSRuntime,
    // Keep info alive for the entire duration of the lifetime of rt
    info: Option<CString>,
}

/// Entry point of the library.
#[derive(Debug, Clone)]
pub struct Runtime {
    pub(crate) inner: Arc<Mutex<Inner>>,
}

impl Runtime {
    pub fn new() -> Result<Self, Error> {
        let rt = unsafe { qjs::JS_NewRuntime() };
        if rt == ptr::null_mut() {
            return Err(Error::Allocation);
        }
        Ok(Runtime {
            inner: Arc::new(Mutex::new(Inner { rt, info: None })),
        })
    }

    pub fn set_info<S: Into<Vec<u8>>>(&mut self, info: S) -> Result<(), Error> {
        let mut guard = self.inner.lock().unwrap();
        let string = CString::new(info)?;
        unsafe { qjs::JS_SetRuntimeInfo(guard.rt, string.as_ptr()) }
        guard.info = Some(string);
        Ok(())
    }

    pub fn set_memory_limit(&self, limit: usize) {
        let guard = self.inner.lock().unwrap();
        let limit = limit as qjs::size_t;
        unsafe { qjs::JS_SetMemoryLimit(guard.rt, limit) }
        mem::drop(guard);
    }

    pub fn set_gc_threshold(&self, threshold: usize) {
        let guard = self.inner.lock().unwrap();
        let threshold = threshold as qjs::size_t;
        unsafe { qjs::JS_SetGCThreshold(guard.rt, threshold) }
        mem::drop(guard);
    }

    pub fn run_gc(&self) {
        let guard = self.inner.lock().unwrap();
        unsafe { qjs::JS_RunGC(guard.rt) }
        mem::drop(guard);
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        unsafe { qjs::JS_FreeRuntime(self.rt) }
    }
}

// Since all functions which use runtime are behind a mutex
// sending the runtime to other threads should be fine.
unsafe impl Send for Runtime {}

// Since a global lock needs to be locked for safe use
// using runtime in a sync way should be safe as
// simultanious accesses is syncronized behind a lock.
unsafe impl Sync for Runtime {}

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
