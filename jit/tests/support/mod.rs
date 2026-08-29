use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use rquickjs_core::runtime::{JitBackend, JitBackendAttachError, RuntimeJitGuard};
use rquickjs_core::Runtime;

use crate::abi::{AbiInfo, AbiMismatch, AbiStructure};
use crate::{Jit, JitConfig, JitDiagnosticKind, JitError};

#[derive(Clone)]
pub struct LifecycleRecorder {
    events: Arc<Mutex<Vec<&'static str>>>,
}

pub struct LifecycleRuntime {
    _guard: RuntimeJitGuard,
    runtime: Runtime,
}

impl LifecycleRuntime {
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }
}

struct RecordingBackend {
    events: Arc<Mutex<Vec<&'static str>>>,
}

unsafe impl JitBackend for RecordingBackend {
    fn runtime_detach(&mut self) {
        self.events.lock().unwrap().push("detach");
    }
}

impl Drop for RecordingBackend {
    fn drop(&mut self) {
        self.events.lock().unwrap().push("backend_drop");
    }
}

pub fn record_lifecycle() -> LifecycleRecorder {
    LifecycleRecorder {
        events: Arc::new(Mutex::new(Vec::new())),
    }
}

impl LifecycleRecorder {
    pub fn runtime(&self) -> LifecycleRuntime {
        let runtime = Runtime::new().expect("test runtime");
        let drop_events = Arc::clone(&self.events);
        runtime.set_jit_runtime_drop_probe(move || {
            drop_events.lock().unwrap().push("runtime_drop");
        });
        let guard = RuntimeJitGuard::attach(
            &runtime,
            RecordingBackend {
                events: Arc::clone(&self.events),
            },
        )
        .expect("attach test backend");
        self.events.lock().unwrap().push("attach");
        LifecycleRuntime {
            _guard: guard,
            runtime,
        }
    }

    pub fn snapshot(&self) -> Vec<&'static str> {
        self.events.lock().unwrap().clone()
    }

    pub fn take(&self) -> Vec<&'static str> {
        let mut events = self.events.lock().unwrap();
        core::mem::take(&mut *events)
    }
}

struct DetachLabelBackend {
    events: Arc<Mutex<Vec<&'static str>>>,
    label: &'static str,
}

unsafe impl JitBackend for DetachLabelBackend {
    fn runtime_detach(&mut self) {
        self.events.lock().unwrap().push(self.label);
    }
}

pub fn duplicate_attachment_is_rejected() -> bool {
    let runtime = Runtime::new().expect("test runtime");
    let events = Arc::new(Mutex::new(Vec::new()));
    let first = RuntimeJitGuard::attach(
        &runtime,
        DetachLabelBackend {
            events: Arc::clone(&events),
            label: "first_detach",
        },
    )
    .expect("first attachment");
    let second = RuntimeJitGuard::attach(
        &runtime,
        DetachLabelBackend {
            events: Arc::clone(&events),
            label: "second_detach",
        },
    );
    let rejected = matches!(second, Err(JitBackendAttachError::AlreadyAttached));
    drop(first);
    rejected && *events.lock().unwrap() == ["first_detach"]
}

#[derive(Clone, Copy, Debug)]
pub enum AbiMismatchFixture {
    SourceRevision,
    OpcodeFingerprint,
    ValueLayout,
    FeatureFlags,
    PointerWidth,
    Endianness,
    AbiInfoLayout,
    FunctionIdLayout,
    HotEventLayout,
    FunctionSnapshotLayout,
    EntryHandleLayout,
    BackendVTableLayout,
}

impl AbiMismatchFixture {
    pub const ALL: [Self; 12] = [
        Self::SourceRevision,
        Self::OpcodeFingerprint,
        Self::ValueLayout,
        Self::FeatureFlags,
        Self::PointerWidth,
        Self::Endianness,
        Self::AbiInfoLayout,
        Self::FunctionIdLayout,
        Self::HotEventLayout,
        Self::FunctionSnapshotLayout,
        Self::EntryHandleLayout,
        Self::BackendVTableLayout,
    ];

    const fn mismatch(self) -> AbiMismatch {
        match self {
            Self::SourceRevision => AbiMismatch::SourceRevision,
            Self::OpcodeFingerprint => AbiMismatch::OpcodeFingerprint,
            Self::ValueLayout => AbiMismatch::ValueLayout,
            Self::FeatureFlags => AbiMismatch::FeatureFlags,
            Self::PointerWidth => AbiMismatch::PointerWidth,
            Self::Endianness => AbiMismatch::Endianness,
            Self::AbiInfoLayout => AbiMismatch::StructureLayout(AbiStructure::AbiInfo),
            Self::FunctionIdLayout => AbiMismatch::StructureLayout(AbiStructure::FunctionId),
            Self::HotEventLayout => AbiMismatch::StructureLayout(AbiStructure::HotEvent),
            Self::FunctionSnapshotLayout => {
                AbiMismatch::StructureLayout(AbiStructure::FunctionSnapshot)
            }
            Self::EntryHandleLayout => AbiMismatch::StructureLayout(AbiStructure::EntryHandle),
            Self::BackendVTableLayout => AbiMismatch::StructureLayout(AbiStructure::BackendVTable),
        }
    }
}

pub fn mismatch_is_rejected_before_attach(fixture: AbiMismatchFixture) -> bool {
    let runtime = Runtime::new().expect("test runtime");
    let mut info = AbiInfo::query_linked().expect("linked ABI");
    info.corrupt(fixture.mismatch());
    let diagnostics = Arc::new(AtomicUsize::new(0));
    let diagnostic_count = Arc::clone(&diagnostics);
    let config = JitConfig::builder()
        .diagnostic_callback(move |diagnostic| {
            if matches!(diagnostic.kind(), JitDiagnosticKind::AbiMismatch(_)) {
                diagnostic_count.fetch_add(1, Ordering::SeqCst);
            }
        })
        .build()
        .unwrap();
    let rejected = matches!(
        Jit::attach_with_info(&runtime, config, info),
        Err(JitError::Abi(_))
    ) && diagnostics.load(Ordering::SeqCst) == 1;
    if !rejected {
        return false;
    }

    // A valid attachment succeeding immediately afterward proves the rejected
    // fixture never stored its vtable in the runtime.
    Jit::attach(&runtime, JitConfig::default()).is_ok()
}
