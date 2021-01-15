use crate::qjs;

/// The type of identifier of class
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "classes")))]
#[repr(transparent)]
pub struct ClassId(qjs::JSClassID);

impl ClassId {
    pub const fn new() -> Self {
        Self(0)
    }

    pub fn get(&self) -> qjs::JSClassID {
        self.0
    }

    pub fn init(&mut self) {
        unsafe { qjs::JS_NewClassID(&mut self.0) };
    }
}
