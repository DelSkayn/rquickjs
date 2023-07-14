use crate::{
    function::{Flat, Opt, Rest, This},
    qjs, Ctx, FromJs, Function, IntoJs, Result, Value,
};

use super::{ffi::defer_call_job, Constructor};

const ARGS_ON_STACK: usize = 4;

pub enum ArgsSlice {
    Stack {
        slice: [qjs::JSValue; ARGS_ON_STACK],
        offset: u8,
    },
    Heap(Vec<qjs::JSValue>),
}

/// Argument input for a functions
///
/// Arguments on the rust side for calling into the JavaScript context.
pub struct Args<'js> {
    ctx: Ctx<'js>,
    pub(crate) this: qjs::JSValue,
    pub(crate) args: ArgsSlice,
}

impl<'js> Args<'js> {
    /// Returns a new args with space for a give number of arguments
    pub fn new(ctx: Ctx<'js>, args: usize) -> Args {
        Args {
            ctx,
            this: qjs::JS_UNDEFINED,
            args: if args <= ARGS_ON_STACK {
                ArgsSlice::Stack {
                    slice: [qjs::JS_UNDEFINED; ARGS_ON_STACK],
                    offset: 0,
                }
            } else {
                ArgsSlice::Heap(Vec::with_capacity(args))
            },
        }
    }

    /// Returns a new args with space for any number of arguments
    pub fn new_unsized(ctx: Ctx<'js>) -> Args {
        Args {
            ctx,
            this: qjs::JS_UNDEFINED,
            args: ArgsSlice::Heap(Vec::new()),
        }
    }

    /// Returns the context associated with these arguments.
    pub fn ctx(&self) -> &Ctx<'js> {
        &self.ctx
    }

    /// Add an argument to the list.
    pub fn push_arg<T: IntoJs<'js>>(&mut self, arg: T) -> Result<()> {
        let v = arg.into_js(&self.ctx)?;

        match self.args {
            ArgsSlice::Stack {
                ref mut slice,
                ref mut offset,
            } => {
                if *offset >= 8 {
                    panic!("pushed more arguments than num_args returned");
                }
                slice[*offset as usize] = v.into_js_value();
                *offset += 1;
            }
            ArgsSlice::Heap(ref mut h) => h.push(v.into_js_value()),
        }

        Ok(())
    }

    /// Add multiple arguments to the list.
    pub fn push_args<T, I>(&mut self, iter: I) -> Result<()>
    where
        T: IntoJs<'js>,
        I: IntoIterator<Item = T>,
    {
        for a in iter.into_iter() {
            self.push_arg(a)?
        }
        Ok(())
    }

    /// Add a this arguments.
    pub fn this<T>(&mut self, this: T) -> Result<()>
    where
        T: IntoJs<'js>,
    {
        let v = this.into_js(&self.ctx)?;
        let v = std::mem::replace(&mut self.this, v.into_js_value());
        unsafe { qjs::JS_FreeValue(self.ctx.as_ptr(), v) };
        Ok(())
    }

    /// Replace the this value with 'Undefined' and return the original value.
    pub fn take_this(&mut self) -> Value<'js> {
        let value = std::mem::replace(&mut self.this, qjs::JS_UNDEFINED);
        Value {
            ctx: self.ctx().clone(),
            value,
        }
    }

    /// The number of arguments currently in the list.
    fn len(&self) -> usize {
        match self.args {
            ArgsSlice::Stack { offset, .. } => offset as usize,
            ArgsSlice::Heap(ref h) => h.len(),
        }
    }

    fn as_ptr(&self) -> *const qjs::JSValue {
        match self.args {
            ArgsSlice::Stack { ref slice, .. } => slice.as_ptr(),
            ArgsSlice::Heap(ref h) => h.as_ptr(),
        }
    }

    /// Call a function with the current set of arguments.
    pub fn apply<R>(self, func: &Function<'js>) -> Result<R>
    where
        R: FromJs<'js>,
    {
        let val = unsafe {
            let val = qjs::JS_Call(
                self.ctx.as_ptr(),
                func.as_js_value(),
                self.this,
                self.len() as _,
                self.as_ptr() as _,
            );
            let val = self.ctx.handle_exception(val)?;
            Value::from_js_value(self.ctx.clone(), val)
        };
        R::from_js(&self.ctx, val)
    }

    pub fn defer(mut self, func: Function<'js>) -> Result<()> {
        let this = self.take_this();
        self.push_arg(this)?;
        self.push_arg(func)?;
        let ctx = self.ctx();
        unsafe {
            if qjs::JS_EnqueueJob(
                ctx.as_ptr(),
                Some(defer_call_job),
                self.len() as _,
                self.as_ptr() as _,
            ) < 0
            {
                return Err(ctx.raise_exception());
            }
        }
        Ok(())
    }

    pub fn construct<R>(self, constructor: &Constructor<'js>) -> Result<R>
    where
        R: FromJs<'js>,
    {
        let value = if unsafe { qjs::JS_VALUE_GET_TAG(self.this) != qjs::JS_TAG_UNDEFINED } {
            unsafe {
                qjs::JS_CallConstructor2(
                    self.ctx.as_ptr(),
                    constructor.as_js_value(),
                    self.this,
                    self.len() as _,
                    self.as_ptr() as _,
                )
            }
        } else {
            unsafe {
                qjs::JS_CallConstructor(
                    self.ctx.as_ptr(),
                    constructor.as_js_value(),
                    self.len() as _,
                    self.as_ptr() as _,
                )
            }
        };
        let value = unsafe { self.ctx.handle_exception(value)? };
        let v = unsafe { Value::from_js_value(self.ctx.clone(), value) };
        R::from_js(&self.ctx, v)
    }
}

impl Drop for Args<'_> {
    fn drop(&mut self) {
        match self.args {
            ArgsSlice::Heap(ref h) => h.iter().for_each(|v| {
                unsafe { qjs::JS_FreeValue(self.ctx.as_ptr(), *v) };
            }),
            ArgsSlice::Stack { ref slice, offset } => {
                slice[..(offset as usize)].iter().for_each(|v| {
                    unsafe { qjs::JS_FreeValue(self.ctx.as_ptr(), *v) };
                })
            }
        }
        unsafe { qjs::JS_FreeValue(self.ctx.as_ptr(), self.this) };
    }
}

/// A trait for converting values into arguments.
pub trait IntoArg<'js> {
    /// The number of arguments this value produces.
    fn num_args(&self) -> usize;

    /// Convert the value into an argument.
    fn into_arg(self, args: &mut Args<'js>) -> Result<()>;
}

/// A trait for converting a tuple of values into a list arguments.
pub trait IntoArgs<'js> {
    /// The number of arguments this value produces.
    fn num_args(&self) -> usize;

    /// Convert the value into an argument.
    fn into_args(self, args: &mut Args<'js>) -> Result<()>;

    fn apply<R>(self, function: &Function<'js>) -> Result<R>
    where
        R: FromJs<'js>,
        Self: Sized,
    {
        let mut args = Args::new(function.ctx().clone(), self.num_args());
        self.into_args(&mut args)?;
        args.apply(function)
    }

    fn defer<R>(self, function: Function<'js>) -> Result<()>
    where
        Self: Sized,
    {
        let mut args = Args::new(function.ctx().clone(), self.num_args());
        self.into_args(&mut args)?;
        args.defer(function)
    }

    fn construct<R>(self, function: &Constructor<'js>) -> Result<()>
    where
        Self: Sized,
    {
        let mut args = Args::new(function.ctx().clone(), self.num_args());
        self.into_args(&mut args)?;
        args.construct(function)
    }
}

impl<'js, T: IntoJs<'js>> IntoArg<'js> for T {
    fn num_args(&self) -> usize {
        1
    }

    fn into_arg(self, args: &mut Args<'js>) -> Result<()> {
        args.push_arg(self)
    }
}

impl<'js, T: IntoJs<'js>> IntoArg<'js> for This<T> {
    fn num_args(&self) -> usize {
        0
    }

    fn into_arg(self, args: &mut Args<'js>) -> Result<()> {
        args.this(self.0)
    }
}

impl<'js, T: IntoJs<'js>> IntoArg<'js> for Opt<T> {
    fn num_args(&self) -> usize {
        self.0.is_some() as usize
    }

    fn into_arg(self, args: &mut Args<'js>) -> Result<()> {
        if let Some(x) = self.0 {
            args.push_arg(x)?
        }
        Ok(())
    }
}

impl<'js, T: IntoJs<'js>> IntoArg<'js> for Rest<T> {
    fn num_args(&self) -> usize {
        self.0.len()
    }

    fn into_arg(self, args: &mut Args<'js>) -> Result<()> {
        args.push_args(self.0)
    }
}

impl<'js, T: IntoArgs<'js>> IntoArg<'js> for Flat<T> {
    fn num_args(&self) -> usize {
        self.0.num_args()
    }

    fn into_arg(self, args: &mut Args<'js>) -> Result<()> {
        self.0.into_args(args)
    }
}

macro_rules! impl_into_args {
    ($($t:ident),*) => {
        #[allow(non_snake_case)]
        impl<'js $(,$t)*> IntoArgs<'js> for ($($t,)*)
        where
            $($t : IntoArg<'js>,)*
        {
            fn num_args(&self) -> usize{
                let ($(ref $t,)*) = *self;
                0 $(+ $t.num_args())*
            }

            fn into_args(self, _args: &mut Args<'js>) -> Result<()>{
                let ($($t,)*) = self;
                $($t.into_arg(_args)?;)*
                Ok(())
            }
        }
    };
}

impl_into_args!();
impl_into_args!(A);
impl_into_args!(A, B);
impl_into_args!(A, B, C);
impl_into_args!(A, B, C, D);
impl_into_args!(A, B, C, D, E);
impl_into_args!(A, B, C, D, E, F);
impl_into_args!(A, B, C, D, E, F, G);
