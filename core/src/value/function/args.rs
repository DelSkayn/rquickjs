use crate::{
    function::{Opt, Rest, This},
    qjs, Ctx, Error, FromJs, Result, Value,
};
use std::{ops::Range, slice};

pub struct Input<'js> {
    ctx: Ctx<'js>,
    this: qjs::JSValue,
    args: &'js [qjs::JSValue],
}

impl<'js> Input<'js> {
    #[inline]
    pub unsafe fn new_raw(
        ctx: *mut qjs::JSContext,
        this: qjs::JSValue,
        argc: qjs::c_int,
        argv: *mut qjs::JSValue,
    ) -> Self {
        let ctx = Ctx::from_ptr(ctx);
        let args = slice::from_raw_parts(argv, argc as _);
        Self { ctx, this, args }
    }

    #[inline]
    pub fn access(&self) -> InputAccessor<'_, 'js> {
        InputAccessor {
            input: self,
            arg: 0,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.args.len()
    }
}

pub struct InputAccessor<'i, 'js> {
    input: &'i Input<'js>,
    arg: usize,
}

impl<'i, 'js> InputAccessor<'i, 'js> {
    /// Get context
    #[inline]
    pub fn ctx(&self) -> Ctx<'js> {
        self.input.ctx
    }

    /// Get value of `this`
    #[inline]
    pub fn this<T>(&self) -> Result<T>
    where
        T: FromJs<'js>,
    {
        let value = unsafe { Value::from_js_value_const(self.input.ctx, self.input.this) };
        T::from_js(self.input.ctx, value)
    }

    /// Get count of rest arguments
    #[inline]
    pub fn len(&self) -> usize {
        self.input.args.len() - self.arg
    }

    /// Get next argument
    #[inline]
    pub fn arg<T>(&mut self) -> Result<T>
    where
        T: FromJs<'js>,
    {
        if self.arg < self.input.args.len() {
            let value = self.input.args[self.arg];
            self.arg += 1;
            let value = unsafe { Value::from_js_value_const(self.input.ctx, value) };
            T::from_js(self.input.ctx, value)
        } else {
            Err(Error::new_from_js_message(
                "uninitialized",
                "value",
                "out of range",
            ))
        }
    }

    /// Get rest arguments
    #[inline]
    pub fn args<T>(&mut self) -> Result<Vec<T>>
    where
        T: FromJs<'js>,
    {
        self.input.args[self.arg..]
            .iter()
            .map(|value| {
                let value = unsafe { Value::from_js_value_const(self.input.ctx, *value) };
                T::from_js(self.input.ctx, value)
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Get something
    #[inline]
    pub fn get<T>(&mut self) -> Result<T>
    where
        T: FromInput<'js>,
    {
        T::from_input(self)
    }
}

pub trait FromInput<'js>: Sized {
    /// Get possible number of arguments
    fn num_args() -> Range<usize>;

    /// Get it from input
    fn from_input<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self>;
}

/// Get context from input
impl<'js> FromInput<'js> for Ctx<'js> {
    fn num_args() -> Range<usize> {
        0..0
    }

    fn from_input<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        Ok(accessor.ctx())
        //Ok(Ctx::from_ptr(accessor.ctx().ctx))
    }
}

/// Get the `this` from input
impl<'js, T> FromInput<'js> for This<T>
where
    T: FromJs<'js>,
{
    fn num_args() -> Range<usize> {
        0..0
    }

    fn from_input<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        accessor.this().map(Self)
    }
}

/// Get the next optional argument from input
impl<'js, T> FromInput<'js> for Opt<T>
where
    T: FromJs<'js>,
{
    fn num_args() -> Range<usize> {
        0..1
    }

    fn from_input<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        if accessor.len() > 0 {
            accessor.arg().map(Self)
        } else {
            Ok(Self(None))
        }
    }
}

/// Get the rest arguments from input
impl<'js, T> FromInput<'js> for Rest<T>
where
    T: FromJs<'js>,
{
    fn num_args() -> Range<usize> {
        0..usize::MAX
    }

    fn from_input<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        accessor.args().map(Self)
    }
}

/// Get the next argument from input
impl<'js, T> FromInput<'js> for T
where
    T: FromJs<'js>,
{
    fn num_args() -> Range<usize> {
        1..1
    }

    fn from_input<'i>(accessor: &mut InputAccessor<'i, 'js>) -> Result<Self> {
        accessor.arg()
    }
}
