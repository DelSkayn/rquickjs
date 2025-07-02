use core::{marker::PhantomData, ptr::NonNull};

#[cfg(feature = "futures")]
use crate::{context::AsyncContext, runtime::AsyncRuntime};
use crate::{qjs, util::Sealed, Context, Result, Runtime};

/// The internal trait to add JS builtins
pub trait Intrinsic: Sealed {
    /// # Safety
    /// Do not need implement it yourself instead you may use predefined intrinsics from [`intrinsic`] module.
    unsafe fn add_intrinsic(ctx: NonNull<qjs::JSContext>);
}

/// Used for building a [`Context`](struct.Context.html) with a specific set of intrinsics
pub struct ContextBuilder<I>(PhantomData<I>);

macro_rules! intrinsic_impls {
    (@builtin: $($(#[$meta:meta])* $name:ident $func:ident $(($($args:expr),*))*,)*) => {
        $(
            $(#[$meta])*
            pub struct $name;
            impl crate::util::Sealed for $name { }

            impl Intrinsic for $name {
                unsafe fn add_intrinsic(ctx: NonNull<qjs::JSContext>) {
                    qjs::$func(ctx.as_ptr() $(, $($args),*)*);
                }
            }
        )*
    };

    (@tuple: $($($name:ident)*,)*) => {
        $(
            impl<$($name,)*> crate::util::Sealed for ($($name,)*) { }

            impl<$($name,)*> Intrinsic for ($($name,)*)
            where
                $($name: Intrinsic,)*
            {
                unsafe fn add_intrinsic(_ctx: NonNull<qjs::JSContext>) {
                    $($name::add_intrinsic(_ctx);)*
                }
            }
        )*
    }
}

/// A marker types for intrinsic
///
/// You can select just you need only. If `lto = true` any unused code will be drop by link-time optimizer.
pub mod intrinsic {
    use super::{qjs, Intrinsic, NonNull};

    intrinsic_impls! {
        @builtin:
        /// Add Date object support
        Date JS_AddIntrinsicDate,
        /// Add evaluation support
        Eval JS_AddIntrinsicEval,
        /// Add RegExp compiler
        RegExpCompiler JS_AddIntrinsicRegExpCompiler,
        /// Add RegExp object support
        RegExp JS_AddIntrinsicRegExp,
        /// Add JSON parse and stringify
        Json JS_AddIntrinsicJSON,
        /// Add Proxy object support
        Proxy JS_AddIntrinsicProxy,
        /// Add MapSet object support
        MapSet JS_AddIntrinsicMapSet,
        /// Add Typed Arrays support
        TypedArrays JS_AddIntrinsicTypedArrays,
        /// Add Promise object support
        Promise JS_AddIntrinsicPromise,
        /// Add BigInt support
        BigInt JS_AddIntrinsicBigInt,
        /// Add Performance support
        Performance JS_AddPerformance,
        /// Add WeakRef support
        WeakRef JS_AddIntrinsicWeakRef,
    }

    /// Add none intrinsics
    pub type None = ();

    /// Add all intrinsics
    pub type All = (
        Date,
        Eval,
        RegExpCompiler,
        RegExp,
        Json,
        Proxy,
        MapSet,
        TypedArrays,
        Promise,
        BigInt,
        Performance,
        WeakRef,
    );
}

intrinsic_impls! {
    @tuple:
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
    A B C D E F G H I J K L M N O P R,
}

impl Default for ContextBuilder<()> {
    fn default() -> Self {
        ContextBuilder(PhantomData)
    }
}

impl<I: Intrinsic> ContextBuilder<I> {
    pub fn with<J: Intrinsic>(self) -> ContextBuilder<(I, J)> {
        ContextBuilder(PhantomData)
    }

    pub fn build(self, runtime: &Runtime) -> Result<Context> {
        Context::custom::<I>(runtime)
    }

    #[cfg(feature = "futures")]
    pub async fn build_async(self, runtime: &AsyncRuntime) -> Result<AsyncContext> {
        AsyncContext::custom::<I>(runtime).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_intrinsinces() {
        let rt = crate::Runtime::new().unwrap();
        let ctx = Context::builder()
            .with::<intrinsic::All>()
            .build(&rt)
            .unwrap();
        let result: usize = ctx.with(|ctx| ctx.eval("1+1")).unwrap();
        assert_eq!(result, 2);
    }
}
