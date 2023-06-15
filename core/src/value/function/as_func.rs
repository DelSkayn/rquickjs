use super::{FromInput, Input};
use crate::{
    class,
    function::{Method, MutFn, OnceFn, SelfMethod, This},
    Ctx, Error, FromJs, Function, IntoJs, Result, Value,
};
use std::ops::Range;

#[cfg(feature = "classes")]
use crate::class::{Class, ClassDef, Constructor};

#[cfg(feature = "futures")]
use crate::{function::Async, promise::Promised};
#[cfg(feature = "futures")]
use std::future::Future;

/// The trait to wrap rust function to JS directly
pub trait AsFunction<'js, A, R> {
    /// The possible range of function arguments
    fn num_args() -> Range<usize>;

    /// Call as JS function
    fn call(&self, input: &Input<'js>) -> Result<Value<'js>>;

    /// Post-processing the function
    fn post<'js_>(_ctx: Ctx<'js_>, _func: &Function<'js_>) -> Result<()> {
        Ok(())
    }
}

impl<'js> Input<'js> {
    #[doc(hidden)]
    #[inline]
    pub fn check_num_args<F: AsFunction<'js, A, R>, A, R>(&self) -> Result<()> {
        let expected = F::num_args();
        let given = self.len();
        // We can't simply use Range::contains() because actually we operates with Range as with RangeInclusive
        if expected.start <= given && given <= expected.end {
            Ok(())
        } else {
            Err(Error::new_num_args(expected, given))
        }
    }
}

macro_rules! as_function_impls {
    ($($(#[$meta:meta])* $($arg:ident)*,)*) => {
        $(
            // for Fn()
            $(#[$meta])*
            impl<'js, F, R $(, $arg)*> AsFunction<'js, ($($arg,)*), R> for F
            where
                F: Fn($($arg),*) -> R + 'js,
                R: IntoJs<'js>,
                $($arg: FromInput<'js>,)*
            {
                #[allow(non_snake_case)]
                fn num_args() -> Range<usize> {
                    $(let $arg = $arg::num_args();)*
                    0usize $(+ $arg.start)* .. 0usize $(.saturating_add($arg.end))*
                }

                #[allow(unused_mut)]
                fn call(&self, input: &Input<'js>) -> Result<Value<'js>> {
                    input.check_num_args::<Self, _, _>()?;
                    let mut accessor = input.access();
                    self(
                        $($arg::from_input(&mut accessor)?,)*
                    ).into_js(accessor.ctx())
                }
            }

            // for async Fn() via Async wrapper
            #[cfg(feature = "futures")]
            $(#[$meta])*
            impl<'js, F,Fut, R $(, $arg)*> AsFunction<'js, ($($arg,)*), Promised<R>> for Async<F>
            where
                F: Fn($($arg),*) -> Fut + 'js,
                Fut: Future<Output = Result<R>> + 'js,
                R: IntoJs<'js> + 'js,
                $($arg: FromInput<'js>,)*
            {
                #[allow(non_snake_case)]
                fn num_args() -> Range<usize> {
                    $(let $arg = $arg::num_args();)*
                    0usize $(+ $arg.start)* .. 0usize $(.saturating_add($arg.end))*
                }

                #[allow(unused_mut)]
                fn call(&self, input: &Input<'js>) -> Result<Value<'js>> {
                    input.check_num_args::<Self, _, _>()?;
                    let mut accessor = input.access();
                    Promised(self(
                        $($arg::from_input(&mut accessor)?,)*
                    )).into_js(accessor.ctx())
                }
            }

            // for FnMut() via MutFn wrapper
            $(#[$meta])*
            impl<'js, F, R $(, $arg)*> AsFunction<'js, ($($arg,)*), R> for MutFn<F>
            where
                F: FnMut($($arg),*) -> R + 'js,
                R: IntoJs<'js>,
                $($arg: FromInput<'js>,)*
            {
                #[allow(non_snake_case)]
                fn num_args() -> Range<usize> {
                    $(let $arg = $arg::num_args();)*
                    0usize $(+ $arg.start)* .. 0usize $(.saturating_add($arg.end))*
                }

                #[allow(unused_mut)]
                fn call(&self, input: &Input<'js>) -> Result<Value<'js>> {
                    input.check_num_args::<Self, _, _>()?;
                    let mut func = self.try_borrow_mut()
                        .expect("Mutable function callback is already in use! Could it have been called recursively?");
                    let mut accessor = input.access();
                    func(
                        $($arg::from_input(&mut accessor)?,)*
                    ).into_js(accessor.ctx())
                }
            }

            // for async FnMut() via MutFn wrapper
            #[cfg(feature = "futures")]
            $(#[$meta])*
            impl<'js, F, Fut, R $(, $arg)*> AsFunction<'js, ($($arg,)*), Promised<R>> for Async<MutFn<F>>
            where
                F: FnMut($($arg),*) -> Fut  + 'js,
                Fut: Future<Output = Result<R>> + 'js,
                R: IntoJs<'js> + 'js,
                $($arg: FromInput<'js>,)*
            {
                #[allow(non_snake_case)]
                fn num_args() -> Range<usize> {
                    $(let $arg = $arg::num_args();)*
                    0usize $(+ $arg.start)* .. 0usize $(.saturating_add($arg.end))*
                }

                #[allow(unused_mut)]
                fn call(&self, input: &Input<'js>) -> Result<Value<'js>> {
                    input.check_num_args::<Self, _, _>()?;
                    let mut func = self.try_borrow_mut()
                        .expect("Mutable function callback is already in use! Could it have been called recursively?");
                    let mut accessor = input.access();
                    Promised(func(
                        $($arg::from_input(&mut accessor)?,)*
                    )).into_js(accessor.ctx())
                }
            }

            // for FnOnce() via OnceFn wrapper
            $(#[$meta])*
            impl<'js, F, R $(, $arg)*> AsFunction<'js, ($($arg,)*), R> for OnceFn<F>
            where
                F: FnOnce($($arg),*) -> R + 'js,
                R: IntoJs<'js>,
                $($arg: FromInput<'js>,)*
            {
                #[allow(non_snake_case)]
                fn num_args() -> Range<usize> {
                    $(let $arg = $arg::num_args();)*
                    0usize $(+ $arg.start)* .. 0usize $(.saturating_add($arg.end))*
                }

                #[allow(unused_mut)]
                fn call(&self, input: &Input<'js>) -> Result<Value<'js>> {
                    input.check_num_args::<Self, _, _>()?;
                    let mut func = self.try_borrow_mut()
                        .expect("Once function callback is already in use! Could it have been called recursively?");
                    let func = func.take()
                        .expect("Once function callback is already was used! Could it have been called twice?");
                    let mut accessor = input.access();
                    func(
                        $($arg::from_input(&mut accessor)?,)*
                    ).into_js(accessor.ctx())
                }
            }

            // for async FnOnce() via OnceFn wrapper
            #[cfg(feature = "futures")]
            $(#[$meta])*
            impl<'js, F,Fut, R $(, $arg)*> AsFunction<'js, ($($arg,)*), Promised<R>> for Async<OnceFn<F>>
            where
                F: FnOnce($($arg),*) -> Fut + 'js,
                Fut: Future<Output = Result<R>> + 'js,
                R: IntoJs<'js> + 'js,
                $($arg: FromInput<'js>,)*
            {
                #[allow(non_snake_case)]
                fn num_args() -> Range<usize> {
                    $(let $arg = $arg::num_args();)*
                    0usize $(+ $arg.start)* .. 0usize $(.saturating_add($arg.end))*
                }

                #[allow(unused_mut)]
                fn call(&self, input: &Input<'js>) -> Result<Value<'js>> {
                    input.check_num_args::<Self, _, _>()?;
                    let mut func = self.try_borrow_mut()
                        .expect("Once function callback is already in use! Could it have been called recursively?");
                    let func = func.take()
                        .expect("Once function callback is already was used! Could it have been called twice?");
                    let mut accessor = input.access();
                    Promised(func(
                        $($arg::from_input(&mut accessor)?,)*
                    )).into_js(accessor.ctx())
                }
            }

            // for methods via Method wrapper
            $(#[$meta])*
            impl<'js, F, R, T $(, $arg)*> AsFunction<'js, (T, $($arg),*), R> for Method<F>
            where
                F: Fn(T, $($arg),*) -> R + 'js,
                R: IntoJs<'js>,
                T: FromJs<'js>,
                $($arg: FromInput<'js>,)*
            {
                #[allow(non_snake_case)]
                fn num_args() -> Range<usize> {
                    $(let $arg = $arg::num_args();)*
                    0usize $(+ $arg.start)* .. 0usize $(.saturating_add($arg.end))*
                }

                #[allow(unused_mut)]
                fn call(&self, input: &Input<'js>) -> Result<Value<'js>> {
                    input.check_num_args::<Self, _, _>()?;
                    let mut accessor = input.access();
                    let this = This::<T>::from_input(&mut accessor)?.0;
                    self(
                        this,
                        $($arg::from_input(&mut accessor)?,)*
                    ).into_js(accessor.ctx())
                }
            }

            // for methods via Method wrapper
            $(#[$meta])*
            impl<'js, F, R, T $(, $arg)*> AsFunction<'js, (T, $($arg),*), R> for SelfMethod<T,F>
            where
                F: Fn(&T, $($arg),*) -> R + 'js,
                R: IntoJs<'js>,
                T: ClassDef,
                $($arg: FromInput<'js>,)*
            {
                #[allow(non_snake_case)]
                fn num_args() -> Range<usize> {
                    $(let $arg = $arg::num_args();)*
                    0usize $(+ $arg.start)* .. 0usize $(.saturating_add($arg.end))*
                }

                #[allow(unused_mut)]
                fn call(&self, input: &Input<'js>) -> Result<Value<'js>> {
                    input.check_num_args::<Self, _, _>()?;
                    let mut accessor = input.access();
                    let this = This::<class::Ref<T>>::from_input(&mut accessor)?.0;
                    self(
                        &*this,
                        $($arg::from_input(&mut accessor)?,)*
                    ).into_js(accessor.ctx())
                }
            }

            // for async methods via Method wrapper
            #[cfg(feature = "futures")]
            #[allow(non_snake_case)]
            $(#[$meta])*
            impl<'js,F,Fut,R, T $(, $arg)*> AsFunction<'js, (T, $($arg),*), Promised<R>> for Async<Method<F>>
            where
                F: Fn(T,$($arg),*) -> Fut + 'js,
                Fut: Future<Output = Result<R>> + 'js,
                R: IntoJs<'js> + 'js,
                T: FromJs<'js> + 'js,
                $($arg: FromInput<'js> + 'js,)*
            {
                fn num_args() -> Range<usize> {
                    $(let $arg = $arg::num_args();)*
                    0usize $(+ $arg.start)* .. 0usize $(.saturating_add($arg.end))*
                }

                #[allow(unused_mut)]
                fn call(&self, input: &Input<'js>) -> Result<Value<'js>> {
                    input.check_num_args::<Self, _, _>()?;
                    let mut accessor = input.access();
                    let this = This::<T>::from_input(&mut accessor)?;
                    $(let $arg = $arg::from_input(&mut accessor)?;)*
                    let future = self(
                        this.0,
                        $($arg,)*
                    );
                    Promised(future).into_js(accessor.ctx())
                }
            }

            // for async methods via Method wrapper
            #[cfg(feature = "futures")]
            #[allow(non_snake_case)]
            $(#[$meta])*
            impl<'js,Fut,R, T $(, $arg)*> AsFunction<'js, (&T, $($arg),*), Promised<R>> for Async<SelfMethod<T,fn(&T$(,$arg)*) -> Fut>>
            where
                Fut: Future<Output = Result<R>> + 'js,
                R: IntoJs<'js> + 'js,
                T: ClassDef + 'js,
                $($arg: FromInput<'js> + 'js,)*
            {
                fn num_args() -> Range<usize> {
                    $(let $arg = $arg::num_args();)*
                    0usize $(+ $arg.start)* .. 0usize $(.saturating_add($arg.end))*
                }

                #[allow(unused_mut)]
                fn call(&self, input: &Input<'js>) -> Result<Value<'js>> {
                    input.check_num_args::<Self, _, _>()?;
                    let mut accessor = input.access();
                    let this = This::<class::Ref<T>>::from_input(&mut accessor)?;
                    $(let $arg = $arg::from_input(&mut accessor)?;)*
                    let f = self.0.0;
                    let future = async move {
                        f(
                            &*this,
                            $($arg,)*
                        ).await
                    };
                    Promised(future).into_js(accessor.ctx())
                }
            }

            // for async methods via Method wrapper
            #[cfg(feature = "futures")]
            #[allow(non_snake_case)]
            $(#[$meta])*
            impl<'js,Fut,R, T $(, $arg)*> AsFunction<'js, (&T, $($arg),*), Promised<R>> for Async<SelfMethod<T,fn(T$(,$arg)*) -> Fut>>
            where
                Fut: Future<Output = Result<R>> + 'js,
                R: IntoJs<'js> + 'js,
                T: ClassDef + Clone + 'js,
                $($arg: FromInput<'js> + 'js,)*
            {
                fn num_args() -> Range<usize> {
                    $(let $arg = $arg::num_args();)*
                    0usize $(+ $arg.start)* .. 0usize $(.saturating_add($arg.end))*
                }

                #[allow(unused_mut)]
                fn call(&self, input: &Input<'js>) -> Result<Value<'js>> {
                    input.check_num_args::<Self, _, _>()?;
                    let mut accessor = input.access();
                    let this = This::<class::Ref<T>>::from_input(&mut accessor)?;
                    $(let $arg = $arg::from_input(&mut accessor)?;)*
                    let f = self.0.0;
                    let future = async move {
                        f(
                            (**this).clone(),
                            $($arg,)*
                        ).await
                    };
                    Promised(future).into_js(accessor.ctx())
                }
            }

        )*
    };
}

as_function_impls! {
    ,
    A,
    A B,
    A B D,
    A B D E,
    A B D E G,
    A B D E G H,
    #[cfg(feature = "max-args-7")]
    A B C D E G H I,
    #[cfg(feature = "max-args-8")]
    A B C D E G H I J,
    #[cfg(feature = "max-args-9")]
    A B C D E G H I J K,
    #[cfg(feature = "max-args-10")]
    A B C D E G H I J K L,
    #[cfg(feature = "max-args-11")]
    A B C D E G H I J K L M,
    #[cfg(feature = "max-args-12")]
    A B C D E G H I J K L M N,
    #[cfg(feature = "max-args-13")]
    A B C D E G H I J K L M N O,
    #[cfg(feature = "max-args-14")]
    A B C D E G H I J K L M N O P,
    #[cfg(feature = "max-args-15")]
    A B C D E G H I J K L M N O P U,
    #[cfg(feature = "max-args-16")]
    A B C D E G H I J K L M N O P U V,
}

// for constructors via Constructor wrapper
#[cfg(feature = "classes")]
impl<'js, C, F, A, R> AsFunction<'js, A, R> for Constructor<C, F>
where
    C: ClassDef + 'js,
    F: AsFunction<'js, A, R> + 'js,
{
    fn num_args() -> Range<usize> {
        F::num_args()
    }

    #[allow(unused_mut)]
    fn call(&self, input: &Input<'js>) -> Result<Value<'js>> {
        input.check_num_args::<Self, _, _>()?;

        let mut accessor = input.access();
        let ctx = accessor.ctx();
        let this: Value = accessor.this()?;
        let proto = this
            .as_function()
            // called as a constructor (with new keyword)
            .map(|func| func.get_prototype())
            // called as a function
            .unwrap_or_else(|| {
                if C::HAS_PROTO {
                    Class::<C>::prototype(ctx)
                } else {
                    // Fallback to the a ordinary object as prototype.
                    // more correct would be the %Object.prototype% as defined by ecma but we dont have
                    // access to fundamental objects.
                    crate::Object::new(ctx)
                }
            })?;
        // call constructor
        let res = self.0.call(input)?;
        // set prototype to support inheritance
        res.as_object()
            .ok_or_else(|| Error::new_into_js(res.type_of().as_str(), C::CLASS_NAME))?
            .set_prototype(&proto)?;
        Ok(res)
    }

    fn post<'js_>(ctx: Ctx<'js_>, func: &Function<'js_>) -> Result<()> {
        func.set_constructor(true);
        let proto = if C::HAS_PROTO {
            Class::<C>::prototype(ctx)?
        } else {
            // Fallback to the a ordinary object as prototype.
            // more correct would be the %Object.prototype% as defined by ecma but we dont have
            // access to fundamental objects.
            crate::Object::new(ctx)?
        };
        func.set_prototype(&proto);
        Class::<C>::static_init(func)?;
        Ok(())
    }
}

macro_rules! overloaded_impls {
    ($($(#[$meta:meta])* $func:ident<$func_args:ident, $func_res:ident> $($funcs:ident <$funcs_args:ident, $funcs_res:ident>)*,)*) => {
        $(
            $(#[$meta])*
            impl<'js, $func, $func_args, $func_res $(, $funcs, $funcs_args, $funcs_res)*> AsFunction<'js, ($func_args $(, $funcs_args)*), ($func_res $(, $funcs_res)*)> for ($func $(, $funcs)*)
            where
                $func: AsFunction<'js, $func_args, $func_res> + 'js,
            $($funcs: AsFunction<'js, $funcs_args, $funcs_res> + 'js,)*
            {
                #[allow(non_snake_case)]
                fn num_args() -> Range<usize> {
                    let $func = $func::num_args();
                    $(let $funcs = $funcs::num_args();)*
                    $func.start $(.min($funcs.start))* .. $func.end $(.max($funcs.end))*
                }

                #[allow(non_snake_case)]
                fn call(&self, input: &Input<'js>) -> Result<Value<'js>> {
                    input.check_num_args::<Self, _, _>()?;

                    let ($func $(, $funcs)*) = self;

                    // try the first function
                    $func.call(input)
                        $(.or_else(|error| {
                            if error.is_num_args() || error.is_from_js_to_js() {
                                // in case of mismatch args try the second funcion and so on
                                $funcs.call(input)
                            } else {
                                Err(error)
                            }
                        }))*
                }

                fn post<'js_>(ctx: Ctx<'js_>, func: &Function<'js_>) -> Result<()> {
                    $func::post(ctx, func)?;
                    $($funcs::post(ctx, func)?;)*
                    Ok(())
                }
            }
        )*
    };
}

overloaded_impls! {
    F1<A1, R1> F2<A2, R2>,
    F1<A1, R1> F2<A2, R2> F3<A3, R3>,
    F1<A1, R1> F2<A2, R2> F3<A3, R3> F4<A4, R4>,
    F1<A1, R1> F2<A2, R2> F3<A3, R3> F4<A4, R4> F5<A5, R5>,
    F1<A1, R1> F2<A2, R2> F3<A3, R3> F4<A4, R4> F5<A5, R5> F6<A6, R6>,
}
