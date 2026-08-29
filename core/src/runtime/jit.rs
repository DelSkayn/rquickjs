//! Safe ownership bridge for the versioned QuickJS JIT backend ABI.

use alloc::boxed::Box;
use core::{ffi::c_void, fmt, mem, ptr};

use crate::qjs;

use super::Runtime;

/// A backend registered with one QuickJS runtime.
///
/// # Safety
///
/// Implementations must obey the ownership contracts of `quickjs-jit.h`, must
/// not unwind through a callback, and must not retain borrowed callback
/// arguments beyond the call. QuickJS invokes callbacks only while the owning
/// runtime is locked.
pub unsafe trait JitBackend: Send + 'static {
    fn record_hot(&mut self, _event: &qjs::JSJitHotEvent) -> u32 {
        0
    }

    fn submit_snapshot(&mut self, _snapshot: *mut qjs::JSJitFunctionSnapshot) {}

    fn acquire_entry(&mut self, _id: u64, _generation: u64, _pc: u32) -> qjs::JSJitEntryHandle {
        qjs::JSJitEntryHandle {
            struct_size: mem::size_of::<qjs::JSJitEntryHandle>() as u32,
            reserved: 0,
            entry: ptr::null_mut(),
            pin: ptr::null_mut(),
        }
    }

    fn release_entry(&mut self, _entry: qjs::JSJitEntryHandle) {}

    fn runtime_detach(&mut self) {}

    fn function_retire(&mut self, _id: u64, _generation: u64) {}

    fn memory_used(&self) -> usize {
        0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JitBackendAttachError {
    AlreadyAttached,
    InvalidVTable,
    EngineRejected,
}

impl fmt::Display for JitBackendAttachError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyAttached => f.write_str("a JIT backend is already attached"),
            Self::InvalidVTable => f.write_str("the JIT backend vtable is incompatible"),
            Self::EngineRejected => f.write_str("QuickJS rejected the JIT backend"),
        }
    }
}

struct BackendState {
    backend: Box<dyn JitBackend>,
}

impl BackendState {
    unsafe fn from_opaque<'a>(opaque: *mut c_void) -> &'a mut Self {
        debug_assert!(!opaque.is_null());
        unsafe { &mut *opaque.cast() }
    }
}

unsafe extern "C" fn record_hot(opaque: *mut c_void, event: *const qjs::JSJitHotEvent) -> u32 {
    if event.is_null() {
        return 0;
    }
    let state = unsafe { BackendState::from_opaque(opaque) };
    state.backend.record_hot(unsafe { &*event })
}

unsafe extern "C" fn submit_snapshot(
    opaque: *mut c_void,
    snapshot: *mut qjs::JSJitFunctionSnapshot,
) {
    let state = unsafe { BackendState::from_opaque(opaque) };
    state.backend.submit_snapshot(snapshot);
}

unsafe extern "C" fn acquire_entry(
    opaque: *mut c_void,
    id: u64,
    generation: u64,
    pc: u32,
) -> qjs::JSJitEntryHandle {
    let state = unsafe { BackendState::from_opaque(opaque) };
    state.backend.acquire_entry(id, generation, pc)
}

unsafe extern "C" fn release_entry(opaque: *mut c_void, entry: qjs::JSJitEntryHandle) {
    let state = unsafe { BackendState::from_opaque(opaque) };
    state.backend.release_entry(entry);
}

unsafe extern "C" fn runtime_detach(opaque: *mut c_void, _rt: *mut qjs::JSRuntime) {
    let state = unsafe { BackendState::from_opaque(opaque) };
    state.backend.runtime_detach();
}

unsafe extern "C" fn function_retire(opaque: *mut c_void, id: u64, generation: u64) {
    let state = unsafe { BackendState::from_opaque(opaque) };
    state.backend.function_retire(id, generation);
}

unsafe extern "C" fn memory_used(opaque: *mut c_void) -> qjs::size_t {
    let state = unsafe { BackendState::from_opaque(opaque) };
    state
        .backend
        .memory_used()
        .try_into()
        .unwrap_or(qjs::size_t::MAX)
}

static BACKEND_VTABLE: qjs::JSJitBackendVTable = qjs::JSJitBackendVTable {
    struct_size: mem::size_of::<qjs::JSJitBackendVTable>() as u32,
    record_hot: Some(record_hot),
    submit_snapshot: Some(submit_snapshot),
    acquire_entry: Some(acquire_entry),
    release_entry: Some(release_entry),
    runtime_detach: Some(runtime_detach),
    function_retire: Some(function_retire),
    memory_used: Some(memory_used),
};

/// Owns one backend attachment and keeps its runtime alive through detachment.
pub struct RuntimeJitGuard {
    runtime: Runtime,
    backend: Option<Box<BackendState>>,
}

impl RuntimeJitGuard {
    pub fn attach<B>(runtime: &Runtime, backend: B) -> Result<Self, JitBackendAttachError>
    where
        B: JitBackend,
    {
        let mut state = Box::new(BackendState {
            backend: Box::new(backend),
        });
        let opaque = (&mut *state as *mut BackendState).cast::<c_void>();

        {
            let mut raw = runtime.inner.lock();
            unsafe { raw.attach_jit_backend(&BACKEND_VTABLE, opaque)? };
        }

        Ok(Self {
            runtime: runtime.clone(),
            backend: Some(state),
        })
    }
}

impl fmt::Debug for RuntimeJitGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeJitGuard").finish_non_exhaustive()
    }
}

impl Drop for RuntimeJitGuard {
    fn drop(&mut self) {
        let detached = {
            let mut raw = self.runtime.inner.lock();
            unsafe { raw.detach_jit_backend() }
        };

        if detached.is_ok() {
            drop(self.backend.take());
        } else if let Some(backend) = self.backend.take() {
            // The C runtime may still hold this pointer. Leaking on an
            // impossible engine rejection is safer than creating a dangling
            // callback target.
            mem::forget(backend);
        }
        debug_assert!(detached.is_ok(), "QuickJS rejected JIT backend detach");
    }
}
