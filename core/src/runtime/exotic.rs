use crate::qjs;
use alloc::boxed::Box;

#[derive(Debug)]
#[repr(transparent)]
pub(crate) struct ExoticMethodsHolder(*mut qjs::JSClassExoticMethods);

impl ExoticMethodsHolder {
    pub fn new() -> Self {
        Self(Box::into_raw(Box::new(qjs::JSClassExoticMethods {
            get_own_property: None,       // TODO: Implement
            get_own_property_names: None, // TODO: Implement
            delete_property: Some(crate::class::ffi::exotic_delete_property),
            define_own_property: None, // TODO: Implement
            has_property: Some(crate::class::ffi::exotic_has_property),
            set_property: Some(crate::class::ffi::exotic_set_property),
            get_property: Some(crate::class::ffi::exotic_get_property),
        })))
    }

    pub(crate) fn as_ptr(&self) -> *mut qjs::JSClassExoticMethods {
        self.0
    }
}

impl Drop for ExoticMethodsHolder {
    fn drop(&mut self) {
        let _ = unsafe { Box::from_raw(self.0) };
    }
}
