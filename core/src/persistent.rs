use crate::{
    qjs, value::Constructor, Array, BigInt, Ctx, Error, FromJs, Function, IntoJs, Object, Result,
    String, Symbol, Value,
};
use std::{
    fmt,
    mem::{self, ManuallyDrop},
};

/// The trait to help break lifetime rules when JS objects leaves current context via [`Persistent`] wrapper.
///
/// # Safety
///
/// `Outlive<'js>` must be implemented for types the same, specific, lifetime 'js.
/// For example the following is unsound:
/// ```no_run
/// # use rquickjs::Outlive;
/// struct Container<'js>(rquickjs::Object<'js>);
///
/// unsafe impl<'a,'js> Outlive<'js> for Container<'a>{
///     type Target<'to> = Container<'to>;
/// }
/// ```
/// Instead it must be implemented as following
/// ```
/// # use rquickjs::Outlive;
/// struct Container<'js>(rquickjs::Object<'js>);
///
/// unsafe impl<'js> Outlive<'js> for Container<'js>{
///     type Target<'to> = Container<'to>;
/// }
/// ```
/// `Outlive::Target` must be the same type with all 'js lifetimes changed from 'js to 'to, no
/// other lifetimes may be changed and the type must be otherwise the exact same type.
///
pub unsafe trait Outlive<'js> {
    /// The target which has the same type as a `Self` but with another lifetime `'t`
    type Target<'to>;
}

macro_rules! outlive_impls {
    ($($type:ident,)*) => {
        $(
            unsafe impl<'js> Outlive<'js> for $type<'js> {
                type Target<'to> = $type<'to>;
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
}

macro_rules! impl_outlive{
    ($($($ty:ident)::+$(<$($g:ident),+>)*),*$(,)?) => {
        $(
            unsafe impl<'js,$($($g,)*)*> Outlive<'js> for $($ty)::*$(<$($g,)*>)*
            where
                  $($($g: Outlive<'js>,)*)*
            {
                type Target<'to> = $($ty)::*$(<$($g::Target<'to>,)*>)*;
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
);

/// The wrapper for JS values to keep it from GC
///
/// For example you can store JS functions for later use.
/// ```
/// # use rquickjs::{Runtime, Context, Persistent, Function};
/// # let rt = Runtime::new().unwrap();
/// # let ctx = Context::full(&rt).unwrap();
/// let func = ctx.with(|ctx| {
///     Persistent::save(&ctx, ctx.eval::<Function, _>("a => a + 1").unwrap())
/// });
/// let res: i32 = ctx.with(|ctx| {
///     let func = func.clone().restore(&ctx).unwrap();
///     func.call((2,)).unwrap()
/// });
/// assert_eq!(res, 3);
/// let res: i32 = ctx.with(|ctx| {
///     let func = func.restore(&ctx).unwrap();
///     func.call((0,)).unwrap()
/// });
/// assert_eq!(res, 1);
/// ```
///
/// It is an error (`Error::UnrelatedRuntime`) to restore the `Persistent` in a
/// context who isn't part of the original `Runtime`.
///
/// NOTE: Be careful and ensure that no persistent links outlives the runtime,
/// otherwise Runtime will abort the process when dropped.
///
#[derive(Eq, PartialEq, Hash)]
pub struct Persistent<T> {
    pub(crate) rt: *mut qjs::JSRuntime,
    pub(crate) value: T,
}

impl<T: Clone> Clone for Persistent<T> {
    fn clone(&self) -> Self {
        Persistent {
            rt: self.rt,
            value: self.value.clone(),
        }
    }
}

impl<T> fmt::Debug for Persistent<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Persistent")
            .field("rt", &self.rt)
            .field("value", &self.value)
            .finish()
    }
}

impl<T> Persistent<T> {
    fn new_raw(rt: *mut qjs::JSRuntime, value: T) -> Self {
        Self { rt, value }
    }

    unsafe fn outlive_transmute<'from, 'to, U>(t: U) -> U::Target<'to>
    where
        U: Outlive<'from>,
    {
        // extreemly unsafe code which should be safe if outlive is implemented correctly.

        // assertion to check if T and T::Target are the same size, they should be.
        // should compile away if they are the same size.
        assert_eq!(mem::size_of::<U>(), mem::size_of::<U::Target<'static>>());
        assert_eq!(mem::align_of::<U>(), mem::align_of::<U::Target<'static>>());

        // union to transmute between two unrelated types
        // Can't use transmute since it is unable to determine the size of both values.
        union Transmute<A, B> {
            a: ManuallyDrop<A>,
            b: ManuallyDrop<B>,
        }
        let data = Transmute::<U, U::Target<'to>> {
            a: ManuallyDrop::new(t),
        };
        unsafe { ManuallyDrop::into_inner(data.b) }
    }

    /// Save the value of an arbitrary type
    pub fn save<'js>(ctx: &Ctx<'js>, val: T) -> Persistent<T::Target<'static>>
    where
        T: Outlive<'js>,
    {
        let outlived: T::Target<'static> =
            unsafe { Self::outlive_transmute::<'js, 'static, T>(val) };
        let ptr = unsafe { qjs::JS_GetRuntime(ctx.as_ptr()) };
        Persistent {
            rt: ptr,
            value: outlived,
        }
    }

    /// Restore the value of an arbitrary type
    pub fn restore<'js>(self, ctx: &Ctx<'js>) -> Result<T::Target<'js>>
    where
        T: Outlive<'static>,
    {
        let ctx_runtime_ptr = unsafe { qjs::JS_GetRuntime(ctx.as_ptr()) };
        if self.rt != ctx_runtime_ptr {
            return Err(Error::UnrelatedRuntime);
        }
        Ok(unsafe { Self::outlive_transmute::<'static, 'js, T>(self.value) })
    }
}

impl<'js, T, R> FromJs<'js> for Persistent<R>
where
    R: Outlive<'static, Target<'js> = T>,
    T: Outlive<'js, Target<'static> = R> + FromJs<'js>,
{
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> Result<Persistent<R>> {
        let value = T::from_js(ctx, value)?;
        Ok(Persistent::save(ctx, value))
    }
}

impl<'js, T> IntoJs<'js> for Persistent<T>
where
    T: Outlive<'static>,
    T::Target<'js>: IntoJs<'js>,
{
    fn into_js(self, ctx: &Ctx<'js>) -> Result<Value<'js>> {
        self.restore(ctx)?.into_js(ctx)
    }
}

#[cfg(feature = "parallel")]
unsafe impl<T> Send for Persistent<T> {}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    #[should_panic(expected = "UnrelatedRuntime")]
    fn different_runtime() {
        let rt1 = Runtime::new().unwrap();
        let ctx = Context::full(&rt1).unwrap();

        let persistent_v = ctx.with(|ctx| {
            let v: Value = ctx.eval("1").unwrap();
            Persistent::save(&ctx, v)
        });

        let rt2 = Runtime::new().unwrap();
        let ctx = Context::full(&rt2).unwrap();
        ctx.with(|ctx| {
            let _ = persistent_v.clone().restore(&ctx).unwrap();
        });
    }

    #[test]
    fn different_context() {
        let rt1 = Runtime::new().unwrap();
        let ctx1 = Context::full(&rt1).unwrap();
        let ctx2 = Context::full(&rt1).unwrap();

        let persistent_v = ctx1.with(|ctx| {
            let v: Object = ctx.eval("({ a: 1 })").unwrap();
            Persistent::save(&ctx, v)
        });

        std::mem::drop(ctx1);

        ctx2.with(|ctx| {
            let obj: Object = persistent_v.clone().restore(&ctx).unwrap();
            assert_eq!(obj.get::<_, i32>("a").unwrap(), 1);
        });
    }

    #[test]
    fn persistent_function() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();

        let func = ctx.with(|ctx| {
            let func: Function = ctx.eval("a => a + 1").unwrap();
            Persistent::save(&ctx, func)
        });

        let res: i32 = ctx.with(|ctx| {
            let func = func.clone().restore(&ctx).unwrap();
            func.call((2,)).unwrap()
        });
        assert_eq!(res, 3);

        let ctx2 = Context::full(&rt).unwrap();
        let res: i32 = ctx2.with(|ctx| {
            let func = func.restore(&ctx).unwrap();
            func.call((0,)).unwrap()
        });
        assert_eq!(res, 1);
    }

    #[test]
    fn persistent_value() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();

        let persistent_v = ctx.with(|ctx| {
            let v: Value = ctx.eval("1").unwrap();
            Persistent::save(&ctx, v)
        });

        ctx.with(|ctx| {
            let v = persistent_v.clone().restore(&ctx).unwrap();
            ctx.globals().set("v", v).unwrap();
            let eq: Value = ctx.eval("v == 1").unwrap();
            assert!(eq.as_bool().unwrap());
        });
    }
}
