use crate::{qjs, Ctx, Value};
use std::slice;

pub struct Args<'a, 'js> {
    ctx: Ctx<'js>,
    function: qjs::JSValue,
    this: qjs::JSValue,
    args: &'a [qjs::JSValue],
}

impl<'a, 'js> Args<'a, 'js> {
    pub(crate) unsafe fn from_ffi(
        ctx: *mut qjs::JSContext,
        function: qjs::JSValue,
        this: qjs::JSValue,
        argc: qjs::c_int,
        argv: *mut qjs::JSValue,
        _flags: qjs::c_int,
    ) -> Self {
        let argc = usize::try_from(argc).expect("invalid argument number");
        let args = slice::from_raw_parts(argv, argc);
        Self {
            ctx: Ctx::from_ptr(ctx),
            function,
            this,
            args,
        }
    }

    /// Returns the context assiociated with call.
    pub fn ctx(&self) -> Ctx<'js> {
        self.ctx
    }

    /// Returns the value on which this function called. i.e. in `bla.foo()` the `foo` value.
    pub fn function(&self) -> Value<'js> {
        unsafe { Value::from_js_value_const(self.ctx, self.function) }
    }

    /// Returns the this on which this function called. i.e. in `bla.foo()` the `bla` value.
    pub fn this(&self) -> Value<'js> {
        unsafe { Value::from_js_value_const(self.ctx, self.function) }
    }

    /// Returns the argument at a given index..
    pub fn arg(&self, index: usize) -> Option<Value<'js>> {
        self.args
            .get(index)
            .map(|arg| unsafe { Value::from_js_value_const(self.ctx, *arg) })
    }

    /// Returns the number of arguments.
    pub fn len(&self) -> usize {
        self.args.len()
    }

    /// Returns if there are no arguments
    pub fn is_empty(&self) -> bool {
        self.args.is_empty()
    }
}
