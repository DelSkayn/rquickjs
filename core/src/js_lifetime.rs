use crate::{
    atom, value::Constructor, Array, Atom, BigInt, CString, Exception, Function, Module, Object,
    Promise, String, Symbol, Value,
};

/// The trait which signifies a type using the rquickjs `'js` lifetime trick for maintaining safety around Javascript values.
///
/// # Safety
///
/// This trait can only be implemented for types which derive a `'js` lifetime from a Javascript
/// value, directly or indirectly.
///
/// All of the base Javascript types used in rquickjs like [`Value`] have a `'js` lifetime. If a
/// type wants to contains one of those types it must define a lifetime generic. This trait is for
/// indicating that that lifetime is one derived from a Javascript value. Rquickjs needs to know
/// about this lifetime so that it is able to ensure safe use of types.
///
/// This trait can be derived with `#[derive(JsLifetime)]` in most cases, however sometimes a manual
/// implementation is required.
///
/// This trait must be implemented correctly, failing to do so will make it possible to create
/// unsound behavior. Correct implementions have the `'js` lifetime in `JsLifetime<'js>` be the
/// same as the lifetime on the container, furthermore the associated type `Changed<'to>` is
/// defined as the exact same type with the only difference being that the `'js` lifetime is now
/// `'to`.
///
/// The following is a correct implementation of the [`JsLifetime`] trait.
/// ```
/// # use rquickjs::JsLifetime;
/// struct Container<'js>(rquickjs::Object<'js>);
///
/// unsafe impl<'js> JsLifetime<'js> for Container<'js>{
///     type Changed<'to> = Container<'to>;
/// }
/// ```
///
/// If a type does not have any lifetimes associated with it or all the lifetimes are `'static`
/// then if is always save to implement `JsLifetime`.
///
/// See correct example for a static type below.
/// ```
/// # use rquickjs::JsLifetime;
/// struct Bytes(Vec<u8>);
///
/// unsafe impl<'js> JsLifetime<'js> for Bytes{
///     type Changed<'to> = Bytes;
/// }
///
/// ```
///
///
/// ## Incorrect examples
///
/// For example the following is unsound!
/// ```no_run
/// # use rquickjs::JsLifetime;
/// struct Container<'js>(rquickjs::Object<'js>);
///
/// unsafe impl<'a,'js> JsLifetime<'js> for Container<'a>{ // WRONG LIFETIME!
///     type Changed<'to> = Container<'to>;
/// }
/// ```
/// `Container` here is defined as having a `'a` lifetime where it should be `'js`.
///
/// The following is also incorrect
///
/// ```no_run
/// # use rquickjs::JsLifetime;
/// // Her 'a is not derived from an Javascript value type, but instead the lifetime of a reference.
/// struct Container<'a,'js>(&'a rquickjs::Object<'js>);
///
/// // Non 'js lifetime marked as a 'js lifetime. Unsound!
/// unsafe impl<'js> JsLifetime<'js> for Container<'js, 'js>{
///     type Changed<'to> = Container<'to,'to>;
/// }
/// ```
/// The lifetime marked must be from an rquickjs type with a defined `<'js>` lifetime, it cannot be a
/// the lifetime of reference!
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
    CString,
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

unsafe impl<'js> JsLifetime<'js> for () {
    type Changed<'to> = ();
}
