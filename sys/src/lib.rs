#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::upper_case_acronyms)]

use std::ptr;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(not(feature = "bindgen"))]
include!(concat!("bindings/", bindings_env!("TARGET"), ".rs"));

#[cfg(target_pointer_width = "64")]
include!("inlines/ptr_64.rs");

#[cfg(target_pointer_width = "32")]
include!("inlines/ptr_32_nan_boxing.rs");

include!("inlines/common.rs");
