use crate::{Args, FromJs, Function, IntoJs, Result, This};

/// A helper trait to pass arguments on a function calls.
pub trait AsArguments<'js> {
    fn apply<R>(self, func: &Function<'js>) -> Result<R>
    where
        R: FromJs<'js>;
}

use std::iter::{empty, once};

macro_rules! as_args_impls {
    ($($($t:ident)*,)*) => {
        $(
            // -this -args
            as_args_impls!(@imp : ; : ; $($t)*);
            // +this -args
            as_args_impls!(@imp T: [This<T>]; : ; $($t)*);
            // -this +args
            as_args_impls!(@imp : ; X: [Args<X>]; $($t)*);
            // +this +args
            as_args_impls!(@imp T: [This<T>]; X: [Args<X>]; $($t)*);
        )*
    };

    (@imp $($pt:ident)*: $([$($pa:tt)*])*; $($st:ident)*: $([$($sa:tt)*])*; $($t:ident)* ) => {
        impl<'js, $($pt,)* $($t,)* $($st,)*> AsArguments<'js> for ($($($pa)*,)* $($t,)* $($($sa)*,)*)
        where
            $($pt: IntoJs<'js>,)*
            $($t: IntoJs<'js>,)*
            $($st: IntoJs<'js>,)*
        {
            #[allow(non_snake_case)]
            fn apply<R>(self, func: &Function<'js>) -> Result<R>
            where
                R: FromJs<'js>,
            {
                let _ctx = func.0.ctx;
                let args = empty();
                let ($($pt,)* $($t,)* $($st,)*) = self;
                $(let this = $pt.0.into_js(_ctx);)*
                $(let args = args.chain(once($t.into_js(_ctx)));)*
                $(let args = args.chain($st.0.into_iter().map(|arg| arg.into_js(_ctx)));)*
                let this = as_args_impls!(@this this $($($pa)*),*);
                func.call_raw(this, args)
            }
        }
    };

    (@this $this:ident) => { None };

    (@this $this:ident This<T>) => { Some($this) };
}

as_args_impls!(,);
#[cfg(feature = "max-args-1")]
as_args_impls!(A,);
#[cfg(feature = "max-args-2")]
as_args_impls!(A B,);
#[cfg(feature = "max-args-3")]
as_args_impls!(A B C,);
#[cfg(feature = "max-args-4")]
as_args_impls!(A B C D,);
#[cfg(feature = "max-args-5")]
as_args_impls!(A B C D E,);
#[cfg(feature = "max-args-6")]
as_args_impls!(A B C D E F,);
#[cfg(feature = "max-args-7")]
as_args_impls!(A B C D E F G,);
#[cfg(feature = "max-args-8")]
as_args_impls!(A B C D E F G H,);
#[cfg(feature = "max-args-9")]
as_args_impls!(A B C D E F G H I,);
#[cfg(feature = "max-args-10")]
as_args_impls!(A B C D E F G H I J,);
#[cfg(feature = "max-args-11")]
as_args_impls!(A B C D E F G H I J K,);
#[cfg(feature = "max-args-12")]
as_args_impls!(A B C D E F G H I J K L,);
#[cfg(feature = "max-args-13")]
as_args_impls!(A B C D E F G H I J K L M,);
#[cfg(feature = "max-args-14")]
as_args_impls!(A B C D E F G H I J K L M N,);
#[cfg(feature = "max-args-15")]
as_args_impls!(A B C D E F G H I J K L M N O,);
#[cfg(feature = "max-args-16")]
as_args_impls!(A B C D E F G H I J K L M N O P,);
