use std::marker::PhantomData;

use crate::{markers::Invariant, qjs, Value};

/// A trait for classes for tracing references to quickjs objects.
///
/// Quickjs uses reference counting with tracing to break cycles. As a result implementing this
/// trait incorrectly by not tracing an object cannot result in unsound code. It will however
/// result in memory leaks as the GC will be unable to break cycles which in turn result in cyclic
/// references being kept alive forever.
pub trait Trace<'js> {
    fn trace<'a>(&self, tracer: Tracer<'a, 'js>);
}

/// An object used for tracing references
#[derive(Clone, Copy)]
pub struct Tracer<'a, 'js> {
    rt: *mut qjs::JSRuntime,
    mark_func: qjs::JS_MarkFunc,
    /// This trace should not be able to be used with different runtimes
    _inv: Invariant<'js>,
    /// Marker for acting like a reference so that the tracer can't be stored in an object.
    _marker: PhantomData<&'a ()>,
}

impl<'a, 'js> Tracer<'a, 'js> {
    fn mark(self, value: &Value<'js>) {
        let value = value.as_js_value();
        if unsafe { qjs::JS_VALUE_HAS_REF_COUNT(value) } {
            unsafe { qjs::JS_MarkValue(self.rt, value, self.mark_func) };
        }
    }
}

impl<'js> Trace<'js> for Value<'js> {
    fn trace<'a>(&self, tracer: Tracer<'a, 'js>) {
        tracer.mark(self);
    }
}

macro_rules! trace_impls {

    (primitive: $( $(#[$meta:meta])* $($type:ident)::+$(<$lt:lifetime>)?,)*) => {
        $(
        $(#[$meta])*
        impl<'js> Trace<'js> for $($type)::*$(<$lt>)*{
            fn trace<'a>(&self, _tracer: Tracer<'a,'js>) { }
        }
        )*
    };

    (base: $( $(#[$meta:meta])* $($type:ident)::+,)*) => {
        $(
        $(#[$meta])*
        impl<'js> Trace<'js> for $($type)::*<'js>{
            fn trace<'a>(&self, tracer: Tracer<'a,'js>) {
                self.as_value().trace(tracer)
            }
        }
        )*
    };

    (ref: $($($type:ident)::+,)*) => {
        $(
            impl<'js, T> Trace<'js> for $($type)::*<T>
            where
            T: Trace<'js>,
            {
                fn trace<'a>(&self, tracer: Tracer<'a,'js>) {
                    let this: &T = &self;
                    this.trace(tracer);
                }
            }
        )*
    };

    (tup: $($($type:ident)*,)*) => {
        $(
            impl<'js, $($type),*> Trace<'js> for ($($type,)*)
            where
            $($type: Trace<'js>,)*
            {
                #[allow(non_snake_case)]
                fn trace<'a>(&self, _tracer: Tracer<'a,'js>) {
                    let ($($type,)*) = &self;
                    $($type.trace(_tracer);)*
                }
            }
        )*
    };

    (list: $($(#[$meta:meta])* $($type:ident)::+ $({$param:ident})*,)*) => {
        $(
            $(#[$meta])*
            impl<'js, T $(,$param)*> Trace<'js> for $($type)::*<T $(,$param)*>
            where
            T: Trace<'js>,
            {
                fn trace<'a>(&self, tracer: Tracer<'a,'js>) {
                    for item in self.iter() {
                        item.trace(tracer);
                    }
                }
            }
        )*
    };

    (map: $($(#[$meta:meta])* $($type:ident)::+ $({$param:ident})*,)*) => {
        $(
            $(#[$meta])*
            impl<'js, K, V $(,$param)*> Trace<'js> for $($type)::*<K, V $(,$param)*>
            where
            K: Trace<'js>,
            V: Trace<'js>,
            {
                fn trace<'a>(&self, tracer: Tracer<'a,'js>) {
                    for (key,item) in self.iter() {
                        key.trace(tracer);
                        item.trace(tracer);
                    }
                }
            }
        )*
    };
}

trace_impls! {
    primitive:
    u8,u16,u32,u64,usize,
    i8,i16,i32,i64,isize,
    f32,f64,
    bool,char,
    String,
    crate::Atom<'js>,
    crate::Module<'js>,
}

trace_impls! {
    base:
    crate::Object,
    crate::Array,
    crate::Function,
    crate::BigInt,
    crate::Symbol,
    crate::Exception,
    crate::String,
}

trace_impls! {
    ref:
    Box,
    std::rc::Rc,
    std::sync::Arc,
}

trace_impls! {
    tup:
    ,
    A,
    A B,
    A B C,
    A B C D,
    A B C D E,
    A B C D E F,
    A B C D E F G,
    A B C D E F G H,
    A B C D E F G H I,
    A B C D E F G H I J,
    A B C D E F G H I J K,
    A B C D E F G H I J K L,
    A B C D E F G H I J K L M,
    A B C D E F G H I J K L M N,
    A B C D E F G H I J K L M N O,
    A B C D E F G H I J K L M N O P,
}

trace_impls! {
    list:
    Vec,
    std::collections::VecDeque,
    std::collections::LinkedList,
    std::collections::HashSet {S},
    std::collections::BTreeSet,
    #[cfg(feature = "indexmap")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(all(feature = "classes", feature = "indexmap"))))]
    indexmap::IndexSet {S},
}

trace_impls! {
    map:
    std::collections::HashMap {S},
    std::collections::BTreeMap,
    #[cfg(feature = "indexmap")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(all(feature = "classes", feature = "indexmap"))))]
    indexmap::IndexMap {S},
}
