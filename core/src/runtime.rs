//! QuickJS runtime related types.

mod base;
mod exotic;
pub(crate) mod opaque;
pub(crate) mod raw;
mod userdata;

#[cfg(feature = "jit-abi")]
mod jit;

#[doc(hidden)]
#[cfg(feature = "jit-test-support")]
pub mod test_support;

#[cfg(feature = "futures")]
mod r#async;
#[cfg(feature = "futures")]
pub(crate) mod schedular;
#[cfg(feature = "futures")]
mod spawner;
#[cfg(feature = "futures")]
pub use spawner::DriveFuture;

use alloc::boxed::Box;
pub use base::{Runtime, WeakRuntime};
#[cfg(feature = "jit-abi")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "jit-abi")))]
pub use jit::{JitBackend, JitBackendAttachError, RuntimeJitGuard};
pub use userdata::{UserDataError, UserDataGuard};

#[cfg(feature = "futures")]
pub(crate) use r#async::InnerRuntime;
#[cfg(feature = "futures")]
pub use r#async::{AsyncRuntime, AsyncWeakRuntime};

use crate::value::promise::PromiseHookType;
use crate::{Ctx, Value};

/// The type of the promise hook.
#[cfg(not(feature = "parallel"))]
pub type PromiseHook =
    Box<dyn for<'a> Fn(Ctx<'a>, PromiseHookType, Value<'a>, Value<'a>) + 'static>;
/// The type of the promise hook.
#[cfg(feature = "parallel")]
pub type PromiseHook =
    Box<dyn for<'a> Fn(Ctx<'a>, PromiseHookType, Value<'a>, Value<'a>) + Send + 'static>;

/// The type of the promise rejection tracker.
#[cfg(not(feature = "parallel"))]
pub type RejectionTracker = Box<dyn for<'a> Fn(Ctx<'a>, Value<'a>, Value<'a>, bool) + 'static>;
/// The type of the promise rejection tracker.
#[cfg(feature = "parallel")]
pub type RejectionTracker =
    Box<dyn for<'a> Fn(Ctx<'a>, Value<'a>, Value<'a>, bool) + Send + 'static>;

/// The type of the interrupt handler.
#[cfg(not(feature = "parallel"))]
pub type InterruptHandler = Box<dyn FnMut() -> bool + 'static>;
/// The type of the interrupt handler.
#[cfg(feature = "parallel")]
pub type InterruptHandler = Box<dyn FnMut() -> bool + Send + 'static>;

/// A struct with information about the runtimes memory usage.
pub type MemoryUsage = crate::qjs::JSMemoryUsage;

#[cfg(all(test, feature = "jit-abi"))]
mod test {
    use alloc::sync::Arc;
    use core::{
        mem,
        sync::atomic::{AtomicBool, Ordering},
    };

    use super::{JitBackend, Runtime};
    use crate::qjs;

    struct DetachProbe(Arc<AtomicBool>);

    unsafe impl JitBackend for DetachProbe {
        fn runtime_detach(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn guard_detaches_backend_while_runtime_is_alive() {
        let runtime = Runtime::new().unwrap();
        let detached = Arc::new(AtomicBool::new(false));
        let guard = runtime
            .attach_jit_backend(DetachProbe(Arc::clone(&detached)))
            .unwrap();
        let clone = runtime.clone();

        drop(runtime);
        drop(guard);
        assert!(detached.load(Ordering::SeqCst));
        clone.run_gc();
    }

    #[test]
    fn engine_rejects_a_mismatched_vtable_size() {
        let runtime = Runtime::new().unwrap();
        let raw = runtime.inner.lock();
        let mut vtable = unsafe { mem::zeroed::<qjs::JSJitBackendVTable>() };
        vtable.struct_size = mem::size_of::<qjs::JSJitBackendVTable>() as u32 - 1;
        let status =
            unsafe { qjs::JS_SetJitBackend(raw.rt.as_ptr(), &vtable, core::ptr::null_mut()) };
        assert_eq!(status, qjs::JS_JIT_BACKEND_INVALID_VTABLE);
    }

    #[test]
    fn engine_detach_is_idempotent() {
        let runtime = Runtime::new().unwrap();
        let raw = runtime.inner.lock();
        for _ in 0..2 {
            let status = unsafe {
                qjs::JS_SetJitBackend(raw.rt.as_ptr(), core::ptr::null(), core::ptr::null_mut())
            };
            assert_eq!(status, qjs::JS_JIT_BACKEND_OK);
        }
    }
}
