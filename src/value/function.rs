use crate::{
    value::rf::JsObjectRef, Ctx, FromJs, FromJsMulti, MultiValue, Object, Result, ToJs, ToJsMulti,
    Value,
};
use rquickjs_sys as qjs;
use std::{ffi::CString, mem, os::raw::c_int};

unsafe extern "C" fn call_fn<'js, F>(
    ctx: *mut qjs::JSContext,
    this: qjs::JSValue,
    argc: c_int,
    argv: *mut qjs::JSValue,
) -> qjs::JSValue
where
    F: StaticFn<'js>,
{
    //TODO catch unwind
    let val: Result<Value> = (|| {
        let ctx = Ctx::from_ptr(ctx);
        let this = F::This::from_js(ctx, Value::from_js_value_const(ctx, this)?)?;
        let multi = MultiValue::from_value_count_const(ctx, argc as usize, argv);
        let args = F::Args::from_js_multi(ctx, multi)?;
        let value = F::PTR(ctx, this, args).to_js(ctx)?;
        Ok(value)
    })();
    match val {
        Ok(x) => x.to_js_value(),
        Err(e) => {
            let error = format!("{}", e);
            let error_str = CString::new(error).unwrap();
            qjs::JS_ThrowInternalError(ctx, error_str.as_ptr())
        }
    }
}

pub trait StaticFn<'js> {
    type Args: FromJsMulti<'js>;
    type This: FromJs<'js>;
    type Result: ToJs<'js>;

    const PTR: fn(Ctx<'js>, Self::This, Self::Args) -> Self::Result;
}

/// Rust representation of a javascript function.
#[derive(Debug, Clone, PartialEq)]
pub struct Function<'js>(pub(crate) JsObjectRef<'js>);

impl<'js> Function<'js> {
    pub fn new_static<F, N>(ctx: Ctx<'js>, name: N) -> Result<Self>
    where
        N: Into<Vec<u8>>,
        F: StaticFn<'js>,
    {
        let name = CString::new(name)?;
        let func = call_fn::<F>
            as unsafe extern "C" fn(
                *mut qjs::JSContext,
                qjs::JSValue,
                c_int,
                *mut qjs::JSValue,
            ) -> qjs::JSValue;
        let func: qjs::JSCFunction = Some(func);
        unsafe {
            let val = qjs::JS_NewCFunction2(
                ctx.ctx,
                func,
                name.as_ptr(),
                F::Args::len() as c_int,
                qjs::JSCFunctionEnum_JS_CFUNC_generic,
                0,
            );
            Ok(Function(JsObjectRef::from_js_value(ctx, val)))
        }
    }
    /// Call a function with given arguments with this as the global object.
    ///
    pub fn call<A, R>(&self, args: A) -> Result<R>
    where
        A: ToJsMulti<'js>,
        R: FromJs<'js>,
    {
        let args = args.to_js_multi(self.0.ctx)?;
        let len = args.len();
        let res = unsafe {
            // Dont drop args value
            let mut args: Vec<_> = args.iter().map(|x| x.as_js_value()).collect();
            let val = qjs::JS_Call(
                self.0.ctx.ctx,
                self.0.as_js_value(),
                self.0.ctx.globals().0.as_js_value(),
                len as i32,
                args.as_mut_ptr(),
            );
            R::from_js(self.0.ctx, Value::from_js_value(self.0.ctx, val)?)
        };
        // Make sure the lifetime of args remains valid during the
        // entire duration of the call.
        mem::drop(args);
        res
    }

    pub fn to_object(self) -> Object<'js> {
        Object(self.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::*;
    #[test]
    fn base_call() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let f: Function = ctx.eval("(a) => a + 4").unwrap();
            let res = f.call(3).unwrap();
            println!("{:?}", res);
            assert_eq!(i32::from_js(ctx, res).unwrap(), 7);
            let f: Function = ctx.eval("(a,b) => a * b + 4").unwrap();
            let res = f.call((3, 4)).unwrap();
            println!("{:?}", res);
            assert_eq!(i32::from_js(ctx, res).unwrap(), 16);
        })
    }

    fn test<'js>(_: Ctx<'js>, _: (), _: ()) {
        println!("test")
    }

    struct Test;

    impl<'js> StaticFn<'js> for Test {
        type Args = ();
        type This = ();
        type Result = ();
        const PTR: fn(Ctx<'js>, (), ()) = test;
    }

    #[test]
    fn static_callback() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let f = Function::new_static::<Test, _>(ctx, "test").unwrap();
            let f_2: Function = ctx.eval("() => { 3 }").unwrap();
            let eval: Function = ctx.eval("(a) => { a()}").unwrap();
            eval.call::<_, ()>(f).unwrap();
        })
    }
}
