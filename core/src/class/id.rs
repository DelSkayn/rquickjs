use rquickjs_sys::JSRuntime;

use crate::{
    qjs,
    runtime::opaque::{ClassIdKey, Opaque},
};
use std::sync::atomic::{AtomicUsize, Ordering};

/// The type of identifier of class
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "classes")))]
pub struct ClassId {
    type_id: AtomicUsize,
}

static CLASS_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

impl ClassId {
    /// Create a new class id.
    #[allow(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self {
            type_id: AtomicUsize::new(0),
        }
    }

    /// Get the class Id.
    /// Will initialize itself if it has not done so.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn get(&self, rt: *mut JSRuntime) -> qjs::JSClassID {
        let type_id = self.init_type_id();
        let key = ClassIdKey(rt, type_id);

        let opaque = unsafe { &mut (*qjs::JS_GetRuntimeOpaque(rt).cast::<Opaque>()) };

        let id = opaque.get_class_id_map().entry(key).or_insert_with(|| {
            let mut id = 0;
            unsafe { qjs::JS_NewClassID(rt, &mut id) };
            id
        });
        *id
    }

    /// Initialize the class ID.
    /// Can be called multiple times but will only be initialized once.
    fn init_type_id(&self) -> usize {
        let id: usize = self.type_id.load(Ordering::Relaxed);
        if id == 0 {
            let new_id = CLASS_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
            self.type_id.store(new_id, Ordering::Relaxed);
            return new_id;
        }
        id
    }
}
