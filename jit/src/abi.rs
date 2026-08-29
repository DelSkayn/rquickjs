//! Safe inspection and validation of the linked QuickJS JIT ABI.

use core::{fmt, mem};

use rquickjs_core::qjs;

pub const ABI_MAJOR: u16 = 1;
pub const ABI_MINOR: u16 = 0;

const SOURCE_REVISION: u64 = 0xfd0a_0210_b7be_0095;
const OPCODE_FINGERPRINT: u64 = 0x0054_b0c4_5fd0_91af;
const BUILD_FEATURE_FLAGS: u64 = 0x0000_0000_0000_0001;
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbiStructure {
    AbiInfo,
    FunctionId,
    HotEvent,
    FunctionSnapshot,
    EntryHandle,
    BackendVTable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbiMismatch {
    StructSize,
    Major,
    Minor,
    PointerWidth,
    Endianness,
    ValueSize,
    SourceRevision,
    OpcodeFingerprint,
    ValueLayout,
    FeatureFlags,
    StructureLayout(AbiStructure),
    BuildFingerprint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbiError {
    QueryFailed(i32),
    Incompatible(AbiMismatch),
}

impl fmt::Display for AbiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueryFailed(status) => write!(f, "QuickJS ABI query failed with status {status}"),
            Self::Incompatible(mismatch) => {
                write!(f, "incompatible QuickJS JIT ABI: {mismatch:?}")
            }
        }
    }
}

impl std::error::Error for AbiError {}

#[derive(Clone, Copy, Debug)]
pub struct AbiInfo {
    raw: qjs::JSJitABIInfo,
}

impl AbiInfo {
    pub fn linked() -> Result<Self, AbiError> {
        let info = Self::query_linked()?;
        info.validate()?;
        Ok(info)
    }

    pub const fn major(&self) -> u16 {
        self.raw.major
    }

    pub const fn minor(&self) -> u16 {
        self.raw.minor
    }

    pub const fn pointer_width(&self) -> u8 {
        self.raw.pointer_width
    }

    pub const fn little_endian(&self) -> bool {
        self.raw.little_endian != 0
    }

    pub(crate) fn query_linked() -> Result<Self, AbiError> {
        let mut raw = unsafe { mem::zeroed::<qjs::JSJitABIInfo>() };
        raw.struct_size = mem::size_of::<qjs::JSJitABIInfo>() as u32;
        let status = unsafe { qjs::JS_GetJitABIInfo(&mut raw) };
        if status == qjs::JS_JIT_BACKEND_OK {
            Ok(Self { raw })
        } else {
            Err(AbiError::QueryFailed(status))
        }
    }

    pub(crate) fn validate(&self) -> Result<(), AbiError> {
        let raw = &self.raw;
        let mismatch = if raw.struct_size != mem::size_of::<qjs::JSJitABIInfo>() as u32 {
            Some(AbiMismatch::StructSize)
        } else if raw.major != ABI_MAJOR {
            Some(AbiMismatch::Major)
        } else if raw.minor.cmp(&ABI_MINOR).is_lt() {
            Some(AbiMismatch::Minor)
        } else if raw.pointer_width != usize::BITS as u8 {
            Some(AbiMismatch::PointerWidth)
        } else if (raw.little_endian != 0) != cfg!(target_endian = "little") {
            Some(AbiMismatch::Endianness)
        } else if raw.value_size != mem::size_of::<qjs::JSValue>() as u16 {
            Some(AbiMismatch::ValueSize)
        } else if raw.source_revision != SOURCE_REVISION {
            Some(AbiMismatch::SourceRevision)
        } else if raw.opcode_fingerprint != OPCODE_FINGERPRINT {
            Some(AbiMismatch::OpcodeFingerprint)
        } else if raw.value_layout_fingerprint != value_layout_fingerprint() {
            Some(AbiMismatch::ValueLayout)
        } else if raw.build_feature_flags != BUILD_FEATURE_FLAGS {
            Some(AbiMismatch::FeatureFlags)
        } else if raw.abi_info_layout_fingerprint != abi_info_layout_fingerprint() {
            Some(AbiMismatch::StructureLayout(AbiStructure::AbiInfo))
        } else if raw.function_id_layout_fingerprint != function_id_layout_fingerprint() {
            Some(AbiMismatch::StructureLayout(AbiStructure::FunctionId))
        } else if raw.hot_event_layout_fingerprint != hot_event_layout_fingerprint() {
            Some(AbiMismatch::StructureLayout(AbiStructure::HotEvent))
        } else if raw.function_snapshot_layout_fingerprint != function_snapshot_layout_fingerprint()
        {
            Some(AbiMismatch::StructureLayout(AbiStructure::FunctionSnapshot))
        } else if raw.entry_handle_layout_fingerprint != entry_handle_layout_fingerprint() {
            Some(AbiMismatch::StructureLayout(AbiStructure::EntryHandle))
        } else if raw.backend_vtable_layout_fingerprint != backend_vtable_layout_fingerprint() {
            Some(AbiMismatch::StructureLayout(AbiStructure::BackendVTable))
        } else if raw.build_fingerprint != expected_build_fingerprint() {
            Some(AbiMismatch::BuildFingerprint)
        } else {
            None
        };

        mismatch.map_or(Ok(()), |mismatch| Err(AbiError::Incompatible(mismatch)))
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn corrupt(&mut self, mismatch: AbiMismatch) {
        match mismatch {
            AbiMismatch::StructSize => self.raw.struct_size ^= 1,
            AbiMismatch::Major => self.raw.major ^= 1,
            AbiMismatch::Minor => self.raw.minor = ABI_MINOR.saturating_sub(1),
            AbiMismatch::PointerWidth => self.raw.pointer_width ^= 32,
            AbiMismatch::Endianness => self.raw.little_endian ^= 1,
            AbiMismatch::ValueSize => self.raw.value_size ^= 1,
            AbiMismatch::SourceRevision => self.raw.source_revision ^= 1,
            AbiMismatch::OpcodeFingerprint => self.raw.opcode_fingerprint ^= 1,
            AbiMismatch::ValueLayout => self.raw.value_layout_fingerprint ^= 1,
            AbiMismatch::FeatureFlags => self.raw.build_feature_flags ^= 1,
            AbiMismatch::StructureLayout(AbiStructure::AbiInfo) => {
                self.raw.abi_info_layout_fingerprint ^= 1
            }
            AbiMismatch::StructureLayout(AbiStructure::FunctionId) => {
                self.raw.function_id_layout_fingerprint ^= 1
            }
            AbiMismatch::StructureLayout(AbiStructure::HotEvent) => {
                self.raw.hot_event_layout_fingerprint ^= 1
            }
            AbiMismatch::StructureLayout(AbiStructure::FunctionSnapshot) => {
                self.raw.function_snapshot_layout_fingerprint ^= 1
            }
            AbiMismatch::StructureLayout(AbiStructure::EntryHandle) => {
                self.raw.entry_handle_layout_fingerprint ^= 1
            }
            AbiMismatch::StructureLayout(AbiStructure::BackendVTable) => {
                self.raw.backend_vtable_layout_fingerprint ^= 1
            }
            AbiMismatch::BuildFingerprint => self.raw.build_fingerprint ^= 1,
        }
    }
}

fn hash_u64(mut hash: u64, mut value: u64) -> u64 {
    for _ in 0..8 {
        hash ^= value & 0xff;
        hash = hash.wrapping_mul(FNV_PRIME);
        value >>= 8;
    }
    hash
}

fn layout_start<T>() -> u64 {
    hash_u64(
        hash_u64(FNV_OFFSET, mem::size_of::<T>() as u64),
        mem::align_of::<T>() as u64,
    )
}

fn layout_field(hash: u64, offset: usize, size: usize) -> u64 {
    hash_u64(hash_u64(hash, offset as u64), size as u64)
}

fn function_id_layout_fingerprint() -> u64 {
    let mut hash = layout_start::<qjs::JSJitFunctionId>();
    hash = layout_field(hash, mem::offset_of!(qjs::JSJitFunctionId, struct_size), 4);
    hash = layout_field(hash, mem::offset_of!(qjs::JSJitFunctionId, reserved), 4);
    hash = layout_field(hash, mem::offset_of!(qjs::JSJitFunctionId, id), 8);
    layout_field(hash, mem::offset_of!(qjs::JSJitFunctionId, generation), 8)
}

fn hot_event_layout_fingerprint() -> u64 {
    let mut hash = layout_start::<qjs::JSJitHotEvent>();
    hash = layout_field(hash, mem::offset_of!(qjs::JSJitHotEvent, struct_size), 4);
    hash = layout_field(hash, mem::offset_of!(qjs::JSJitHotEvent, kind), 4);
    hash = layout_field(
        hash,
        mem::offset_of!(qjs::JSJitHotEvent, function),
        mem::size_of::<qjs::JSJitFunctionId>(),
    );
    hash = layout_field(hash, mem::offset_of!(qjs::JSJitHotEvent, pc), 4);
    layout_field(hash, mem::offset_of!(qjs::JSJitHotEvent, count), 4)
}

fn function_snapshot_layout_fingerprint() -> u64 {
    let mut hash = layout_start::<qjs::JSJitFunctionSnapshot>();
    hash = layout_field(
        hash,
        mem::offset_of!(qjs::JSJitFunctionSnapshot, struct_size),
        4,
    );
    hash = layout_field(hash, mem::offset_of!(qjs::JSJitFunctionSnapshot, flags), 4);
    hash = layout_field(
        hash,
        mem::offset_of!(qjs::JSJitFunctionSnapshot, function),
        mem::size_of::<qjs::JSJitFunctionId>(),
    );
    layout_field(
        hash,
        mem::offset_of!(qjs::JSJitFunctionSnapshot, opaque),
        mem::size_of::<*mut core::ffi::c_void>(),
    )
}

fn entry_handle_layout_fingerprint() -> u64 {
    let mut hash = layout_start::<qjs::JSJitEntryHandle>();
    hash = layout_field(hash, mem::offset_of!(qjs::JSJitEntryHandle, struct_size), 4);
    hash = layout_field(hash, mem::offset_of!(qjs::JSJitEntryHandle, reserved), 4);
    hash = layout_field(
        hash,
        mem::offset_of!(qjs::JSJitEntryHandle, entry),
        mem::size_of::<*mut core::ffi::c_void>(),
    );
    layout_field(
        hash,
        mem::offset_of!(qjs::JSJitEntryHandle, pin),
        mem::size_of::<*mut core::ffi::c_void>(),
    )
}

fn backend_vtable_layout_fingerprint() -> u64 {
    let mut hash = layout_start::<qjs::JSJitBackendVTable>();
    hash = layout_field(
        hash,
        mem::offset_of!(qjs::JSJitBackendVTable, struct_size),
        4,
    );
    for offset in [
        mem::offset_of!(qjs::JSJitBackendVTable, record_hot),
        mem::offset_of!(qjs::JSJitBackendVTable, submit_snapshot),
        mem::offset_of!(qjs::JSJitBackendVTable, acquire_entry),
        mem::offset_of!(qjs::JSJitBackendVTable, release_entry),
        mem::offset_of!(qjs::JSJitBackendVTable, runtime_detach),
        mem::offset_of!(qjs::JSJitBackendVTable, function_retire),
        mem::offset_of!(qjs::JSJitBackendVTable, memory_used),
    ] {
        hash = layout_field(hash, offset, mem::size_of::<usize>());
    }
    hash
}

fn abi_info_layout_fingerprint() -> u64 {
    let mut hash = layout_start::<qjs::JSJitABIInfo>();
    for (offset, size) in [
        (mem::offset_of!(qjs::JSJitABIInfo, struct_size), 4),
        (mem::offset_of!(qjs::JSJitABIInfo, major), 2),
        (mem::offset_of!(qjs::JSJitABIInfo, minor), 2),
        (mem::offset_of!(qjs::JSJitABIInfo, pointer_width), 1),
        (mem::offset_of!(qjs::JSJitABIInfo, little_endian), 1),
        (mem::offset_of!(qjs::JSJitABIInfo, value_size), 2),
        (mem::offset_of!(qjs::JSJitABIInfo, source_revision), 8),
        (mem::offset_of!(qjs::JSJitABIInfo, opcode_fingerprint), 8),
        (
            mem::offset_of!(qjs::JSJitABIInfo, value_layout_fingerprint),
            8,
        ),
        (mem::offset_of!(qjs::JSJitABIInfo, build_feature_flags), 8),
        (mem::offset_of!(qjs::JSJitABIInfo, build_fingerprint), 8),
        (
            mem::offset_of!(qjs::JSJitABIInfo, abi_info_layout_fingerprint),
            8,
        ),
        (
            mem::offset_of!(qjs::JSJitABIInfo, function_id_layout_fingerprint),
            8,
        ),
        (
            mem::offset_of!(qjs::JSJitABIInfo, hot_event_layout_fingerprint),
            8,
        ),
        (
            mem::offset_of!(qjs::JSJitABIInfo, function_snapshot_layout_fingerprint),
            8,
        ),
        (
            mem::offset_of!(qjs::JSJitABIInfo, entry_handle_layout_fingerprint),
            8,
        ),
        (
            mem::offset_of!(qjs::JSJitABIInfo, backend_vtable_layout_fingerprint),
            8,
        ),
    ] {
        hash = layout_field(hash, offset, size);
    }
    hash
}

fn value_layout_fingerprint() -> u64 {
    let mut hash = layout_start::<qjs::JSValue>();
    hash = hash_u64(hash, (qjs::JS_TAG_FIRST as i64) as u64);
    hash = hash_u64(hash, (qjs::JS_TAG_FLOAT64 as i64) as u64);
    hash_u64(hash, u64::from(cfg!(target_pointer_width = "32")))
}

fn expected_build_fingerprint() -> u64 {
    let mut hash = hash_u64(FNV_OFFSET, SOURCE_REVISION);
    hash = hash_u64(hash, OPCODE_FINGERPRINT);
    hash = hash_u64(hash, value_layout_fingerprint());
    hash = hash_u64(hash, BUILD_FEATURE_FLAGS);
    hash = hash_u64(hash, abi_info_layout_fingerprint());
    hash = hash_u64(hash, function_id_layout_fingerprint());
    hash = hash_u64(hash, hot_event_layout_fingerprint());
    hash = hash_u64(hash, function_snapshot_layout_fingerprint());
    hash = hash_u64(hash, entry_handle_layout_fingerprint());
    hash_u64(hash, backend_vtable_layout_fingerprint())
}
