use crate::{
    atom, value::Constructor, Array, Atom, BigInt, Exception, Function, Module, Object, Promise,
    String, Symbol, Value,
};

/// The trait to help break lifetime rules when JS objects leaves current context via [`Persistent`] wrapper.
///
/// # Safety
///
/// `JsLifetime<'js>` must be implemented for types the same, specific, lifetime 'js.
/// For example the following is unsound:
/// ```no_run
/// # use rquickjs::JsLifetime;
/// struct Container<'js>(rquickjs::Object<'js>);
///
/// unsafe impl<'a,'js> JsLifetime<'js> for Container<'a>{
///     type Changed<'to> = Container<'to>;
/// }
/// ```
/// Instead it must be implemented as following
/// ```
/// # use rquickjs::JsLifetime;
/// struct Container<'js>(rquickjs::Object<'js>);
///
/// unsafe impl<'js> JsLifetime<'js> for Container<'js>{
///     type Changed<'to> = Container<'to>;
/// }
/// ```
/// `JsLifetime::Changed` must be the same type with all 'js lifetimes changed from 'js to 'to, no
/// other lifetimes may be changed and the type must be otherwise the exact same type.
///
pub unsafe trait JsLifetime<'js> {
    /// The target which has the same type as a `Self` but with another lifetime `'t`
    type Changed<'to>: 'to;
}

macro_rules! outlive_impls {
    ($($type:ident,)*) => {
        $(
            unsafe impl<'js> JsLifetime<'js> for $type<'js> {
                type Changed<'to> = $type<'to>;
                //type Static = $type<'static>;
            }
        )*
    };
}

outlive_impls! {
    Value,
    Symbol,
    String,
    Object,
    Array,
    BigInt,
    Function,
    Constructor,
    Promise,
    Exception,
    Atom,
}

macro_rules! impl_outlive{
    ($($($ty:ident)::+$(<$($g:ident),+>)*),*$(,)?) => {
        $(
            unsafe impl<'js,$($($g,)*)*> JsLifetime<'js> for $($ty)::*$(<$($g,)*>)*
            where
                  $($($g: JsLifetime<'js>,)*)*
            {
                type Changed<'to> = $($ty)::*$(<$($g::Changed<'to>,)*>)*;
                //type Static = $($ty)::*$(<$($g::Static,)*>)*;
            }
        )*
    };
}

impl_outlive!(
    u8,
    u16,
    u32,
    u64,
    usize,
    u128,
    i8,
    i16,
    i32,
    i64,
    isize,
    i128,
    char,
    std::string::String,
    Vec<T>,
    Box<T>,
    Option<T>,
    std::result::Result<T,E>,
    std::backtrace::Backtrace,
    std::cell::Cell<T>,
    std::cell::RefCell<T>,
    std::cell::UnsafeCell<T>,
    std::collections::BTreeMap<K,V>,
    std::collections::BTreeSet<K>,
    std::collections::BinaryHeap<K>,
    std::collections::HashMap<K,V>,
    std::collections::HashSet<K>,
    std::collections::LinkedList<T>,
    std::collections::VecDeque<T>,
    std::ffi::CString,
    std::ffi::OsString,
    std::ops::Range<T>,
    std::ops::RangeFrom<T>,
    std::ops::RangeFull,
    std::ops::RangeInclusive<T>,
    std::ops::RangeTo<T>,
    std::ops::RangeToInclusive<T>,
    std::ops::Bound<T>,
    std::ops::ControlFlow<B,C>,
    std::process::Child,
    std::process::Command,
    std::process::ExitCode,
    std::process::ExitStatus,
    std::process::Output,
    std::process::Stdio,
    std::path::PathBuf,
    std::rc::Rc<T>,
    std::sync::Arc<T>,
    std::sync::Mutex<T>,
    std::sync::RwLock<T>,
    atom::PredefinedAtom,
);

unsafe impl<'js, T: JsLifetime<'js>> JsLifetime<'js> for Module<'js, T> {
    type Changed<'to> = Module<'to, T::Changed<'to>>;
}
