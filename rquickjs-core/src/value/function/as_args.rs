use crate::{qjs, Ctx, FromJs, Function, IntoJs, Opt, Rest, Result, This};

/// The input for function call
pub struct CallInput<'js> {
    ctx: Ctx<'js>,
    pub(crate) this: qjs::JSValue,
    pub(crate) args: Vec<qjs::JSValue>,
}

impl<'js> Drop for CallInput<'js> {
    fn drop(&mut self) {
        let ctx = self.ctx.ctx;
        for arg in &self.args {
            unsafe { qjs::JS_FreeValue(ctx, *arg) }
        }
        unsafe { qjs::JS_FreeValue(ctx, self.this) }
    }
}

impl<'js> CallInput<'js> {
    #[inline]
    pub(crate) fn new(ctx: Ctx<'js>, nargs: usize) -> Self {
        //let this = ctx.globals().into_value().into_js_value();
        let this = qjs::JS_UNDEFINED;
        let args = Vec::with_capacity(nargs);
        Self { ctx, this, args }
    }

    /// Get context
    #[inline]
    pub fn ctx(&self) -> Ctx<'js> {
        self.ctx
    }

    /// Set this value
    #[inline]
    pub fn this<T>(&mut self, this: T) -> Result<()>
    where
        T: IntoJs<'js>,
    {
        let this = this.into_js(self.ctx)?;
        unsafe { qjs::JS_FreeValue(self.ctx.ctx, self.this) };
        self.this = this.into_js_value();
        Ok(())
    }

    /// Add this as an argument
    #[inline]
    pub fn this_arg(&mut self) {
        self.args.push(self.this);
        self.this = qjs::JS_UNDEFINED;
    }

    #[inline]
    /// Add argument
    pub fn arg<T>(&mut self, arg: T) -> Result<()>
    where
        T: IntoJs<'js>,
    {
        let arg = arg.into_js(self.ctx)?;
        self.args.push(arg.into_js_value());
        Ok(())
    }

    /// Add arguments
    pub fn args<T>(&mut self, args: T) -> Result<()>
    where
        T: IntoIterator,
        T::Item: IntoJs<'js>,
    {
        let args = args.into_iter();
        let len = match args.size_hint() {
            (_, Some(max)) => max,
            (min, _) => min,
        };
        if len > 0 {
            self.args.reserve(len);
        }
        for arg in args {
            self.args.push(arg.into_js(self.ctx)?.into_js_value())
        }
        Ok(())
    }
}

/// A helper trait to prepare inputs for function calls
pub trait IntoInput<'js> {
    /// Get number of arguments
    fn num_args(&self) -> usize;

    /// Put the value into inputs
    fn into_input(self, input: &mut CallInput<'js>) -> Result<()>;
}

impl<'js, T> IntoInput<'js> for This<T>
where
    T: IntoJs<'js>,
{
    fn num_args(&self) -> usize {
        0
    }

    fn into_input(self, input: &mut CallInput<'js>) -> Result<()> {
        input.this(self.0)
    }
}

impl<'js, T> IntoInput<'js> for Opt<T>
where
    T: IntoJs<'js>,
{
    fn num_args(&self) -> usize {
        if self.is_some() {
            1
        } else {
            0
        }
    }

    fn into_input(self, input: &mut CallInput<'js>) -> Result<()> {
        if let Some(arg) = self.0 {
            input.arg(arg)
        } else {
            Ok(())
        }
    }
}

impl<'js, T> IntoInput<'js> for Rest<T>
where
    T: IntoJs<'js>,
{
    fn num_args(&self) -> usize {
        self.len()
    }

    fn into_input(self, input: &mut CallInput<'js>) -> Result<()> {
        input.args(self.0)
    }
}

impl<'js, T> IntoInput<'js> for T
where
    T: IntoJs<'js>,
{
    fn num_args(&self) -> usize {
        1
    }

    fn into_input(self, input: &mut CallInput<'js>) -> Result<()> {
        input.arg(self)
    }
}

/// A helper trait to pass arguments on a function calls.
pub trait AsArguments<'js> {
    fn apply<R>(self, func: &Function<'js>) -> Result<R>
    where
        R: FromJs<'js>;

    fn defer_apply(self, func: &Function<'js>) -> Result<()>;
}

macro_rules! as_args_impls {
    ($($($arg:ident)*,)*) => {
        $(
            impl<'js $(, $arg)*> AsArguments<'js> for ($($arg,)*)
            where
                $($arg: IntoInput<'js>,)*
            {
                #[allow(non_snake_case, unused_mut)]
                fn apply<R>(self, func: &Function<'js>) -> Result<R>
                where
                    R: FromJs<'js>,
                {
                    let ctx = func.0.ctx;
                    let ($($arg,)*) = self;
                    let len = 0 $(+ $arg.num_args())*;
                    let mut input = CallInput::new(ctx, len);
                    $($arg.into_input(&mut input)?;)*
                    let res = func.call_raw(&input)?;
                    R::from_js(ctx, res)
                }

                #[allow(non_snake_case, unused_mut)]
                fn defer_apply(self, func: &Function<'js>) -> Result<()> {
                    let ctx = func.0.ctx;
                    let ($($arg,)*) = self;
                    let len = 0 $(+ $arg.num_args())*;
                    let mut input = CallInput::new(ctx, len);
                    $($arg.into_input(&mut input)?;)*
                    func.defer_call_raw(&mut input)
                }
            }
        )*
    };
}

as_args_impls! {
    ,
    A,
    A B,
    A B C,
    A B C D,
    A B C D E,
    A B C D E F,
}
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
