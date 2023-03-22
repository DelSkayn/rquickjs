#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::upper_case_acronyms)]
#![cfg_attr(test, allow(deref_nullptr))] // TODO: Remove it after closing bindgen#1651

use std::ptr;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(not(feature = "bindgen"))]
include!(concat!("bindings/", bindings_env!("TARGET"), ".rs"));

#[cfg(all(target_pointer_width = "64", feature = "box64"))]
include!("inlines/ptr_64_nan_boxing.rs");

#[cfg(all(target_pointer_width = "64", not(feature = "box64")))]
include!("inlines/ptr_64.rs");

#[cfg(target_pointer_width = "32")]
include!("inlines/ptr_32_nan_boxing.rs");

include!("inlines/common.rs");
