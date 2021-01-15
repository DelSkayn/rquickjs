use crate::{qjs, Persistent};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, LinkedList, VecDeque},
    rc::Rc,
    sync::Arc,
};

#[cfg(feature = "indexmap")]
use indexmap::{IndexMap, IndexSet};

#[cfg(feature = "either")]
use either::{Either, Left, Right};

/// The helper trait to mark internal JS value refs
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "classes")))]
pub trait HasRefs {
    fn mark_refs(&self, marker: &RefsMarker);
}

impl<T> HasRefs for Persistent<T> {
    fn mark_refs(&self, marker: &RefsMarker) {
        if marker.rt == self.rt {
            self.mark_raw(marker.mark_func);
        }
    }
}

impl<T> HasRefs for Option<T>
where
    T: HasRefs,
{
    fn mark_refs(&self, marker: &RefsMarker) {
        if let Some(value) = &self {
            value.mark_refs(marker);
        }
    }
}

#[cfg(feature = "either")]
#[cfg_attr(
    feature = "doc-cfg",
    doc(cfg(all(feature = "classes", feature = "either")))
)]
impl<L, R> HasRefs for Either<L, R>
where
    L: HasRefs,
    R: HasRefs,
{
    fn mark_refs(&self, marker: &RefsMarker) {
        match self {
            Left(value) => value.mark_refs(marker),
            Right(value) => value.mark_refs(marker),
        }
    }
}

macro_rules! has_refs_impls {
    (ref: $($type:ident,)*) => {
        $(
            impl<T> HasRefs for $type<T>
            where
                T: HasRefs,
            {
                fn mark_refs(&self, marker: &RefsMarker) {
                    let this: &T = &self;
                    this.mark_refs(marker);
                }
            }
        )*
    };

    (tup: $($($type:ident)*,)*) => {
        $(
            impl<$($type),*> HasRefs for ($($type,)*)
            where
                $($type: HasRefs,)*
            {
                #[allow(non_snake_case)]
                fn mark_refs(&self, _marker: &RefsMarker) {
                    let ($($type,)*) = &self;
                    $($type.mark_refs(_marker);)*
                }
            }
        )*
    };

	  (list: $($(#[$meta:meta])* $type:ident $({$param:ident})*,)*) => {
		    $(
            $(#[$meta])*
            impl<T $(,$param)*> HasRefs for $type<T $(,$param)*>
            where
                T: HasRefs,
            {
                fn mark_refs(&self, marker: &RefsMarker) {
                    for item in self.iter() {
                        item.mark_refs(marker);
                    }
                }
            }
        )*
	  };

    (map: $($(#[$meta:meta])* $type:ident $({$param:ident})*,)*) => {
		    $(
            $(#[$meta])*
            impl<K, V $(,$param)*> HasRefs for $type<K, V $(,$param)*>
            where
                V: HasRefs,
            {
                fn mark_refs(&self, marker: &RefsMarker) {
                    for item in self.values() {
                        item.mark_refs(marker);
                    }
                }
            }
        )*
	  };
}

has_refs_impls! {
    ref:
    Box,
    Rc,
    Arc,
}

has_refs_impls! {
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

has_refs_impls! {
    list:
    Vec,
    VecDeque,
    LinkedList,
    HashSet {S},
    BTreeSet,
    #[cfg(feature = "indexmap")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(all(feature = "classes", feature = "indexmap"))))]
    IndexSet {S},
}

has_refs_impls! {
    map:
    HashMap {S},
    BTreeMap,
    #[cfg(feature = "indexmap")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(all(feature = "classes", feature = "indexmap"))))]
    IndexMap {S},
}

/// The helper for QuickJS garbage collector which helps it find internal JS object references.
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "classes")))]
#[derive(Clone, Copy)]
pub struct RefsMarker {
    pub(crate) rt: *mut qjs::JSRuntime,
    pub(crate) mark_func: qjs::JS_MarkFunc,
}
