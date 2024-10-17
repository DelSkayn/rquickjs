use rquickjs_sys::JSRuntime;

use crate::{qjs, runtime::opaque::Opaque};
use std::sync::atomic::{AtomicU32, Ordering};

/// The type of identifier of class
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "classes")))]
pub struct ClassId {
    type_id: AtomicU32,
}

static CLASS_ID_COUNTER: AtomicU32 = AtomicU32::new(1);

impl ClassId {
    /// Create a new class id.
    #[allow(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self {
            type_id: AtomicU32::new(0),
        }
    }

    /// Get the class Id.
    /// Will initialize itself if it has not done so.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn get(&self, rt: *mut JSRuntime) -> qjs::JSClassID {
        let type_id = self.init_type_id();

        let opaque: &mut Opaque = unsafe { &mut (*qjs::JS_GetRuntimeOpaque(rt).cast::<Opaque>()) };
        opaque.register_class(rt, type_id)
    }

    /// Initialize the class ID.
    /// Can be called multiple times but will only be initialized once.
    fn init_type_id(&self) -> u32 {
        let id = self.type_id.load(Ordering::Relaxed);
        if id == 0 {
            let new_id = CLASS_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
            self.type_id.store(new_id, Ordering::Relaxed);
            return new_id;
        }
        id
    }
}
