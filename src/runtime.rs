use crate::{Error, RegisteryKey};
use fxhash::FxHashSet as HashSet;
use rquickjs_sys as qjs;
use std::{any::Any, ffi::CString, mem};

use crate::{context::Ctx, safe_ref::Ref, value};

/// Opaque book keeping data for rust.
pub struct Opaque {
    /// The registery, used to keep track of which registery values belong to this runtime.
    pub registery: HashSet<RegisteryKey>,
    /// The function callback, used for its finalizer to be able to free closures.
    pub func_class: u32,
    /// Used to carry a panic if a callback triggered one.
    pub panic: Option<Box<dyn Any + Send + 'static>>,
}

impl Opaque {
    fn new() -> Self {
        let mut class_id: u32 = 0;
        unsafe {
            qjs::JS_NewClassID(&mut class_id);
        }
        Opaque {
            registery: HashSet::default(),
            func_class: class_id,
            panic: None,
        }
    }
}

pub(crate) struct Inner {
    pub(crate) rt: *mut qjs::JSRuntime,
    // To keep rt info alive for the entire duration of the lifetime of rt
    info: Option<CString>,
}

pub(crate) type InnerRef = Ref<Inner>;

/// Quickjs runtime, entry point of the library.
#[derive(Clone)]
pub struct Runtime {
    pub(crate) inner: InnerRef,
}

impl Runtime {
    /// Create a new runtime.
    ///
    /// Will generally only fail if not enough memory was available.
    pub fn new() -> Result<Self, Error> {
        let rt = unsafe { qjs::JS_NewRuntime() };
        if rt.is_null() {
            return Err(Error::Allocation);
        }
        let opaque = Opaque::new();
        unsafe {
            qjs::JS_SetRuntimeOpaque(rt, Box::into_raw(Box::new(opaque)) as *mut _);
        }
        Ok(Runtime {
            inner: InnerRef::new(Inner { rt, info: None }),
        })
    }

    /// Set the info of the runtime
    pub fn set_info<S: Into<Vec<u8>>>(&mut self, info: S) -> Result<(), Error> {
        let mut guard = self.inner.lock();
        let string = CString::new(info)?;
        unsafe { qjs::JS_SetRuntimeInfo(guard.rt, string.as_ptr()) }
        guard.info = Some(string);
        Ok(())
    }

    /// Set a limit on the max amount of memory the runtime
    /// will use.
    ///
    /// Setting the limit to 0 is equivalent to unlimited memory.
    pub fn set_memory_limit(&self, limit: usize) {
        let guard = self.inner.lock();
        let limit = limit as qjs::size_t;
        unsafe { qjs::JS_SetMemoryLimit(guard.rt, limit) }
        mem::drop(guard);
    }

    /// Set a memory threshold for garbage collection.
    pub fn set_gc_threshold(&self, threshold: usize) {
        let guard = self.inner.lock();
        let threshold = threshold as qjs::size_t;
        unsafe { qjs::JS_SetGCThreshold(guard.rt, threshold) }
        mem::drop(guard);
    }

    /// Manually run the garbage collection.
    ///
    /// Most of quickjs values are reference counted and
    /// will automaticly free themselfs when they have no more
    /// references. The garbage collector is only for collecting
    /// cyclic references.
    pub fn run_gc(&self) {
        let guard = self.inner.lock();
        unsafe { qjs::JS_RunGC(guard.rt) }
        mem::drop(guard);
    }

    /// Test for pending jobs
    ///
    /// Returns true when at least one job is pending.
    pub fn is_job_pending(&self) -> bool {
        let guard = self.inner.lock();
        let res = unsafe { qjs::JS_IsJobPending(guard.rt) };
        mem::drop(guard);
        res != 0
    }

    /// Execute first pending job
    ///
    /// Returns context for executed job or none when queue is empty or error when exception thrown under execution.
    pub fn execute_pending_job(&self) -> Result<Option<Ctx<'_>>, Error> {
        let guard = self.inner.lock();
        let mut ctx_ptr = mem::MaybeUninit::<*mut qjs::JSContext>::uninit();
        let result = unsafe { qjs::JS_ExecutePendingJob(guard.rt, ctx_ptr.as_mut_ptr()) };
        mem::drop(guard);
        if result == 0 {
            // no jobs executed
            return Ok(None);
        }
        let ctx = Ctx::from_ptr(unsafe { ctx_ptr.assume_init() });
        if result == 1 {
            // single job executed
            return Ok(Some(ctx));
        }
        // exception thrown
        Err(unsafe { value::get_exception(ctx) })
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        unsafe {
            let ptr = qjs::JS_GetRuntimeOpaque(self.rt);
            let _opaque: Box<Opaque> = Box::from_raw(ptr as *mut _);
            qjs::JS_FreeRuntime(self.rt)
        }
    }
}

// Since all functions which use runtime are behind a mutex
// sending the runtime to other threads should be fine.
#[cfg(feature = "parallel")]
unsafe impl Send for Runtime {}

// Since a global lock needs to be locked for safe use
// using runtime in a sync way should be safe as
// simultanious accesses is syncronized behind a lock.
#[cfg(feature = "parallel")]
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
