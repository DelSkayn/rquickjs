use crate::qjs;
use std::{cell::Cell, sync::Once};

/// The type of identifier of class
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "classes")))]
pub struct ClassId {
    id: Cell<qjs::JSClassID>,
    once: Once,
}

unsafe impl Send for ClassId {}
unsafe impl Sync for ClassId {}

impl ClassId {
    /// Create a new class id.
    #[allow(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self {
            id: Cell::new(0),
            once: Once::new(),
        }
    }

    /// Get the class Id.
    /// Will initialize itself if it has not done so.
    pub fn get(&self) -> qjs::JSClassID {
        self.init();
        self.id.get()
    }

    /// Initialize the class ID.
    /// Can be called multiple times but will only be initialized once.
    fn init(&self) {
        self.once.call_once(|| {
            let mut id = 0;
            unsafe { qjs::JS_NewClassID(&mut id) };
            self.id.set(id);
        })
    }
}
