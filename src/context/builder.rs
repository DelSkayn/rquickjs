use crate::{Context, Error, Runtime};
use quickjs_sys as qjs;
use std::mem;

/// Used for building a [`Context`](struct.Context.html) with a specific set of intrinsics
#[derive(Debug)]
pub struct ContextBuilder<'a> {
    date: bool,
    eval: bool,
    string_normalize: bool,
    regex_comp: bool,
    regex: bool,
    json: bool,
    proxy: bool,
    map_set: bool,
    typed_arrays: bool,
    promise: bool,
    big_int: bool,
    big_float: bool,
    big_decimal: bool,
    operator_overloading: bool,
    rf: &'a Runtime,
}

impl<'a> ContextBuilder<'a> {
    pub fn new(rf: &'a Runtime) -> Self {
        ContextBuilder {
            date: false,
            eval: false,
            string_normalize: false,
            regex_comp: false,
            regex: false,
            json: false,
            proxy: false,
            map_set: false,
            typed_arrays: false,
            promise: false,
            big_int: false,
            big_float: false,
            big_decimal: false,
            operator_overloading: false,
            rf,
        }
    }

    /// Enable all intrinsics
    pub fn all(mut self) -> Self {
        self.date = true;
        self.eval = true;
        self.string_normalize = true;
        self.regex_comp = true;
        self.regex = true;
        self.json = true;
        self.proxy = true;
        self.map_set = true;
        self.typed_arrays = true;
        self.promise = true;
        self.big_int = true;
        self.big_float = true;
        self.big_decimal = true;
        self.operator_overloading = true;
        self
    }

    /// disable all intrinsics
    pub fn none(mut self) -> Self {
        self.date = true;
        self.eval = true;
        self.string_normalize = true;
        self.regex_comp = true;
        self.regex = true;
        self.json = true;
        self.proxy = true;
        self.map_set = true;
        self.typed_arrays = true;
        self.promise = true;
        self.big_int = true;
        self.big_float = true;
        self.big_decimal = true;
        self.operator_overloading = true;
        self
    }
    pub fn date(mut self, value: bool) -> Self {
        self.date = value;
        self
    }
    pub fn eval(mut self, value: bool) -> Self {
        self.eval = value;
        self
    }
    pub fn string_normalize(mut self, value: bool) -> Self {
        self.string_normalize = value;
        self
    }
    pub fn regex_comp(mut self, value: bool) -> Self {
        self.regex_comp = value;
        self
    }
    pub fn regex(mut self, value: bool) -> Self {
        self.regex = value;
        self
    }
    pub fn json(mut self, value: bool) -> Self {
        self.json = value;
        self
    }
    pub fn proxy(mut self, value: bool) -> Self {
        self.proxy = value;
        self
    }
    pub fn map_set(mut self, value: bool) -> Self {
        self.map_set = value;
        self
    }
    pub fn typed_arrays(mut self, value: bool) -> Self {
        self.typed_arrays = value;
        self
    }
    pub fn promises(mut self, value: bool) -> Self {
        self.promise = value;
        self
    }
    pub fn big_int(mut self, value: bool) -> Self {
        self.big_int = value;
        self
    }
    pub fn big_float(mut self, value: bool) -> Self {
        self.big_float = value;
        self
    }
    pub fn big_decimal(mut self, value: bool) -> Self {
        self.big_decimal = value;
        self
    }
    pub fn operator_overloading(mut self, value: bool) -> Self {
        self.operator_overloading = value;
        self
    }

    pub fn build(self) -> Result<Context, Error> {
        let ctx = Context::base(self.rf)?;
        let guard = self.rf.inner.lock.lock().unwrap();
        if self.date {
            unsafe { qjs::JS_AddIntrinsicDate(ctx.ctx) };
        }
        if self.eval {
            unsafe { qjs::JS_AddIntrinsicEval(ctx.ctx) };
        }
        if self.string_normalize {
            unsafe { qjs::JS_AddIntrinsicStringNormalize(ctx.ctx) };
        }
        if self.regex_comp {
            unsafe { qjs::JS_AddIntrinsicRegExpCompiler(ctx.ctx) };
        }
        if self.regex {
            unsafe { qjs::JS_AddIntrinsicRegExp(ctx.ctx) };
        }
        if self.json {
            unsafe { qjs::JS_AddIntrinsicJSON(ctx.ctx) };
        }
        if self.proxy {
            unsafe { qjs::JS_AddIntrinsicProxy(ctx.ctx) };
        }
        if self.map_set {
            unsafe { qjs::JS_AddIntrinsicMapSet(ctx.ctx) };
        }
        if self.typed_arrays {
            unsafe { qjs::JS_AddIntrinsicTypedArrays(ctx.ctx) };
        }
        if self.promise {
            unsafe { qjs::JS_AddIntrinsicPromise(ctx.ctx) };
        }
        if self.big_int {
            unsafe { qjs::JS_AddIntrinsicBigInt(ctx.ctx) };
        }
        if self.big_float {
            unsafe { qjs::JS_AddIntrinsicBigFloat(ctx.ctx) };
        }
        if self.big_decimal {
            unsafe { qjs::JS_AddIntrinsicBigDecimal(ctx.ctx) };
        }
        if self.operator_overloading {
            unsafe { qjs::JS_AddIntrinsicOperators(ctx.ctx) };
        }
        // Explicitly drop the guard to ensure it is valid during the entire use of runtime
        mem::drop(guard);
        Ok(ctx)
    }
}
