//#[cfg(not(feature = "macro"))]
mod hand_written;

//#[cfg(not(feature = "macro"))]
use hand_written::NativeModule;

//#[cfg(feature = "macro")]
//mod using_macro;

//#[cfg(feature = "macro")]
//use using_macro::NativeModule;

rquickjs::module_init!(NativeModule);
