use crate::{context::Ctx, ArgsValue, Error, FromJs, IntoJs, RestArgs, Result, Value};
use std::{
    ops::{Deref, DerefMut},
    result::Result as StdResult,
};

#[derive(Clone, Copy, Debug, Default)]
#[repr(transparent)]
pub struct This<T>(pub T);

impl<T> This<T> {
    pub fn into(self) -> T {
        self.0
    }
}

impl<T> From<T> for This<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> AsRef<T> for This<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> AsMut<T> for This<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> Deref for This<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for This<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// The trait to wrap rust function to JS directly
pub trait AsFunction<'js, A, R> {
    /// Minimum number of arguments
    const LEN: u32;

    /// Calling function from JS side
    fn call(&self, ctx: Ctx<'js>, this: Value<'js>, args: ArgsValue<'js>) -> Result<Value<'js>>;
}

/// The trait to wrap rust function to JS directly
pub trait AsFunctionMut<'js, A, R> {
    /// Minimum number of arguments
    const LEN: u32;

    /// Calling function from JS side
    fn call(&mut self, ctx: Ctx<'js>, this: Value<'js>, args: ArgsValue<'js>)
        -> Result<Value<'js>>;
}

macro_rules! as_fn_impls {
    ($($($t:ident)*,)*) => {
        $(
            // for Fn
            as_fn_impls!(@fun [Fn AsFunction &] $($t)*);
            // for FnMut
            as_fn_impls!(@fun [FnMut AsFunctionMut &mut] $($t)*);
        )*
    };

    (@fun [$($f:tt)*] $($t:ident)*) => {
        // -varargs
        as_fn_impls!(@gen [$($f)*] $($t)*; :;);
        // +varargs
        as_fn_impls!(@gen [$($f)*] $($t)*; X: [RestArgs<X>];);
    };

    (@gen [$($f:tt)*] $($t:ident)*; $($s:tt)*) => {
        // -ctx -this
        as_fn_impls!(@imp [$($f)*] $($t)*; :; $($s)*);
        // +ctx -this
        as_fn_impls!(@imp [$($f)*] $($t)*; : [Ctx<'js>]; $($s)*);
        // -ctx +this
        as_fn_impls!(@imp [$($f)*] $($t)*; T: [This<T>]; $($s)*);
        // +ctx +this
        as_fn_impls!(@imp [$($f)*] $($t)*; T: [Ctx<'js>], [This<T>]; $($s)*);
    };

    // $f - closure kind (Fn or FnMut)
    // $i - trait name (AsFunction or AsFunctionMut)
    // $s - self reference (& or &mut)
    // $t - argument type parameters
    // $tp - preceded type parameters
    // $ts - succeeded type parameters
    // $ap - preceded arg types
    // $as - succeeded arg types
    (@imp [$f:tt $i:tt $($s:tt)*] $($t:ident)*; $($tp:ident)*: $([$($ap:tt)*]),*; $($ts:ident)*: $([$($as:tt)*]),*; ) => {
        impl<'js, F, $($tp,)* $($t,)* $($ts,)* R> $i<'js, ($($($ap)*,)* $($t,)* $($($as)*,)*), (R,)> for F
        where
            F: $f($($($ap)*,)* $($t,)* $($($as)*,)*) -> R,
            $($tp: FromJs<'js>,)*
            $($t: FromJs<'js>,)*
            $($ts: FromJs<'js>,)*
            R: IntoJs<'js>,
        {
            const LEN: u32 = 0 $(+ as_fn_impls!(@one $t))*;

            #[allow(unused_mut, unused)]
            fn call($($s)* self, ctx: Ctx<'js>, this: Value<'js>, mut args: ArgsValue<'js>) -> Result<Value<'js>> {
                let mut args = args.iter();
                self($(as_fn_impls!(@arg ctx this args $($ap)*),)* $($t::from_js(ctx, args.next().ok_or_else(|| Error::Unknown)?)?,)* $(as_fn_impls!(@arg ctx this args $($as)*),)*).into_js(ctx)
            }
        }

        impl<'js, F, $($tp,)* $($t,)* $($ts,)* R, Z> $i<'js, ($($($ap)*,)* $($t,)* $($($as)*,)*), (R, Z)> for F
        where
            F: $f($($($ap)*,)* $($t,)* $($($as)*,)*) -> StdResult<R, Z>,
            $($tp: FromJs<'js>,)*
            $($t: FromJs<'js>,)*
            $($ts: FromJs<'js>,)*
            R: IntoJs<'js>,
            Error: From<Z>,
        {
            const LEN: u32 = 0 $(+ as_fn_impls!(@one $t))*;

            #[allow(unused_mut, unused)]
            fn call($($s)* self, ctx: Ctx<'js>, this: Value<'js>, mut args: ArgsValue<'js>) -> Result<Value<'js>> {
                let mut args = args.iter();
                self($(as_fn_impls!(@arg ctx this args $($ap)*),)* $($t::from_js(ctx, args.next().ok_or_else(|| Error::Unknown)?)?,)* $(as_fn_impls!(@arg ctx this args $($as)*),)*).map_err(Error::from)?.into_js(ctx)
            }
        }
    };

    (@arg $ctx:ident $this:ident $args:ident Ctx<'js>) => {
        $ctx
    };

    (@arg $ctx:ident $this:ident $args:ident This<T>) => {
        T::from_js($ctx, $this).map(This)?
    };

    (@arg $ctx:ident $this:ident $args:ident RestArgs<X>) => {
        $args.map(|arg| X::from_js($ctx, arg))
             .collect::<Result<_>>().map(RestArgs)?
    };

    (@one $($t:tt)*) => { 1 };
}

as_fn_impls! {
    ,
    A,
    A B,
    A B C,
    A B C D,
    A B C D E,
    A B C D E G,
    A B C D E G H,
    A B C D E G H I,
    A B C D E G H I J,
    A B C D E G H I J K,
    A B C D E G H I J K L,
    A B C D E G H I J K L M,
    A B C D E G H I J K L M N,
    A B C D E G H I J K L M N O,
    A B C D E G H I J K L M N O P,
}
