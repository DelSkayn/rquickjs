use crate::{
    qjs, value::rf::JsObjectRef, Ctx, FromJs, IntoAtom, IntoJs, Object, Result, SendWhenParallel,
    Value,
};
use std::cell::RefCell;

mod args;
mod as_args;
mod as_func;
mod ffi;
mod types;

use args::ArgsValue;
pub use as_args::AsArguments;
pub use as_func::{AsFunction, AsFunctionMut};
use ffi::FuncOpaque;
pub use types::{Args, Method, This};

/// Rust representation of a javascript function.
#[derive(Debug, Clone, PartialEq)]
pub struct Function<'js>(pub(crate) JsObjectRef<'js>);

impl<'js> Function<'js> {
    pub fn new<'js_, F, A, R, N>(ctx: Ctx<'js>, name: N, func: F) -> Result<Self>
    where
        N: AsRef<str>,
        F: AsFunction<'js_, A, R> + SendWhenParallel + 'static,
    {
        let func = FuncOpaque::new(move |ctx, this, args| func.call(ctx, this, args));
        Self::new_raw(ctx, name, F::LEN, func)
    }

    pub fn new_mut<'js_, F, A, R, N>(ctx: Ctx<'js>, name: N, func: F) -> Result<Self>
    where
        N: AsRef<str>,
        F: AsFunctionMut<'js_, A, R> + SendWhenParallel + 'static,
    {
        let func = RefCell::new(func);
        let func = FuncOpaque::new(move |ctx, this, args| {
            let mut func = func.try_borrow_mut()
                .expect("Mutable function callback is already in use! Could it have been called recursively?");
            func.call(ctx, this, args)
        });
        Self::new_raw(ctx, name, F::LEN, func)
    }

    pub fn new_raw<N>(ctx: Ctx<'js>, name: N, len: u32, func: FuncOpaque) -> Result<Self>
    where
        N: AsRef<str>,
    {
        let len_field = "length".into_atom(ctx);
        let len_value = len.into_js(ctx)?;

        let name_field = "name".into_atom(ctx);
        let name_value = name.as_ref().into_js(ctx)?;

        Ok(Function(unsafe {
            let func_obj = func.to_js_value(ctx);
            // Set the `.length` property
            qjs::JS_DefinePropertyValue(
                ctx.ctx,
                func_obj,
                len_field.atom,
                len_value.into_js_value(),
                qjs::JS_PROP_CONFIGURABLE as _,
            );
            // Set the `.name` property
            qjs::JS_DefinePropertyValue(
                ctx.ctx,
                func_obj,
                name_field.atom,
                name_value.into_js_value(),
                qjs::JS_PROP_CONFIGURABLE as _,
            );
            JsObjectRef::from_js_value(ctx, func_obj)
        }))
    }

    /// Call a function with given arguments
    ///
    /// You can use tuples to pass arguments. The `()` treated as no arguments, the `(arg,)` as a single argument and so on.
    ///
    /// To call function on a given `this` you can pass `This(this)` as a first argument.
    /// By default the global context object will be passed as `this`.
    pub fn call<A, R>(&self, args: A) -> Result<R>
    where
        A: AsArguments<'js>,
        R: FromJs<'js>,
    {
        args.apply(self)
    }

    pub(crate) fn call_raw<I, R>(&self, this: Option<Result<Value<'js>>>, args: I) -> Result<R>
    where
        I: Iterator<Item = Result<Value<'js>>>,
        R: FromJs<'js>,
    {
        let this = this
            .unwrap_or_else(|| Ok(Value::Object(self.0.ctx.globals())))?
            .as_js_value();
        let args = args
            .map(|res| res.map(|arg| arg.into_js_value()))
            .collect::<Result<Vec<_>>>()?;
        let len = args.len();
        let res = unsafe {
            let ctx = self.0.ctx.ctx;
            let func = self.0.as_js_value();
            let val = qjs::JS_Call(ctx, func, this, len as _, args.as_ptr() as _);
            for arg in args {
                qjs::JS_FreeValue(ctx, arg);
            }
            Value::from_js_value(self.0.ctx, val)
        }?;
        R::from_js(self.0.ctx, res)
    }

    pub fn into_object(self) -> Object<'js> {
        Object(self.0)
    }

    pub(crate) unsafe fn init_raw_rt(rt: *mut qjs::JSRuntime) {
        FuncOpaque::register(rt);
    }

    pub unsafe fn init_raw(ctx: *mut qjs::JSContext) {
        Self::init_raw_rt(qjs::JS_GetRuntime(ctx));
    }
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn js_call() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let f: Function = ctx.eval("(a) => a + 4").unwrap();
            let res = (3,).apply(&f).unwrap();
            println!("{:?}", res);
            assert_eq!(i32::from_js(ctx, res).unwrap(), 7);
            let f: Function = ctx.eval("(a,b) => a * b + 4").unwrap();
            let res = (3, 4).apply(&f).unwrap();
            println!("{:?}", res);
            assert_eq!(i32::from_js(ctx, res).unwrap(), 16);
        })
    }

    fn test() {
        println!("test");
    }

    #[test]
    fn static_callback() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let f = Function::new(ctx, "test", test).unwrap();
            let eval: Function = ctx.eval("a => { a() }").unwrap();
            (f.clone(),).apply::<()>(&eval).unwrap();
            f.call::<_, ()>(()).unwrap();

            let name: StdString = f.clone().into_object().get("name").unwrap();
            assert_eq!(name, "test");

            let get_name: Function = ctx.eval("a => a.name").unwrap();
            let name: StdString = get_name.call((f.clone(),)).unwrap();
            assert_eq!(name, "test");
        })
    }

    #[test]
    fn const_callback() {
        use std::sync::{Arc, Mutex};
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let called = Arc::new(Mutex::new(false));
            let called_clone = called.clone();
            let f = Function::new(ctx, "test", move || {
                (*called_clone.lock().unwrap()) = true;
            })
            .unwrap();

            let eval: Function = ctx.eval("a => { a() }").unwrap();
            eval.call::<_, ()>((f.clone(),)).unwrap();
            f.call::<_, ()>(()).unwrap();
            assert!(*called.lock().unwrap());

            let name: StdString = f.clone().into_object().get("name").unwrap();
            assert_eq!(name, "test");

            let get_name: Function = ctx.eval("a => a.name").unwrap();
            let name: StdString = get_name.call((f.clone(),)).unwrap();
            assert_eq!(name, "test");
        })
    }

    #[test]
    fn mutable_callback() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let mut v = 0;
            let f = Function::new_mut(ctx, "test", move || {
                v += 1;
                v
            })
            .unwrap();

            let eval: Function = ctx.eval("a => a()").unwrap();
            assert_eq!(eval.call::<_, i32>((f.clone(),)).unwrap(), 1);
            assert_eq!(eval.call::<_, i32>((f.clone(),)).unwrap(), 2);
            assert_eq!(eval.call::<_, i32>((f.clone(),)).unwrap(), 3);

            let name: StdString = f.clone().into_object().get("name").unwrap();
            assert_eq!(name, "test");

            let get_name: Function = ctx.eval("a => a.name").unwrap();
            let name: StdString = get_name.call((f.clone(),)).unwrap();
            assert_eq!(name, "test");
        })
    }

    #[test]
    #[should_panic(
        expected = "Mutable function callback is already in use! Could it have been called recursively?"
    )]
    fn recursive_mutable_callback() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let mut v = 0;
            let f = Function::new_mut(ctx, "test", move |ctx: Ctx| {
                v += 1;
                ctx.globals()
                    .get::<_, Function>("foo")
                    .unwrap()
                    .call::<_, ()>(())
                    .unwrap();
                v
            })
            .unwrap();
            ctx.globals().set("foo", f.clone()).unwrap();
            f.call::<_, ()>(()).unwrap();
        })
    }

    #[test]
    fn multiple_const_callbacks() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let globals = ctx.globals();
            globals
                .set("one", Function::new(ctx, "id", || 1f64).unwrap())
                .unwrap();
            globals
                .set("neg", Function::new(ctx, "neg", |a: f64| -a).unwrap())
                .unwrap();
            globals
                .set(
                    "add",
                    Function::new(ctx, "add", |a: f64, b: f64| a + b).unwrap(),
                )
                .unwrap();

            let r: f64 = ctx.eval("neg(add(one(), 2))").unwrap();
            assert_eq!(r, -3.0);
        })
    }

    #[test]
    fn mutable_callback_which_can_fail() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let globals = ctx.globals();
            let mut id_alloc = 0;
            globals
                .set(
                    "new_id",
                    Function::new_mut(ctx, "id", move || {
                        id_alloc += 1;
                        if id_alloc < 4 {
                            Ok(id_alloc)
                        } else {
                            Err(Error::Unknown)
                        }
                    })
                    .unwrap(),
                )
                .unwrap();

            let id: u32 = ctx.eval("new_id()").unwrap();
            assert_eq!(id, 1);
            let id: u32 = ctx.eval("new_id()").unwrap();
            assert_eq!(id, 2);
            let id: u32 = ctx.eval("new_id()").unwrap();
            assert_eq!(id, 3);
            let _err = ctx.eval::<u32, _>("new_id()").unwrap_err();
        })
    }

    #[test]
    fn mutable_callback_with_ctx_which_reads_globals() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let globals = ctx.globals();
            let mut id_alloc = 0;
            globals
                .set(
                    "new_id",
                    Function::new_mut(ctx, "id", move |ctx: Ctx| {
                        let initial: Option<u32> = ctx.globals().get("initial_id")?;
                        if let Some(initial) = initial {
                            id_alloc += 1;
                            Ok(id_alloc + initial)
                        } else {
                            Err(Error::Unknown)
                        }
                    })
                    .unwrap(),
                )
                .unwrap();

            let _err = ctx.eval::<u32, _>("new_id()").unwrap_err();
            globals.set("initial_id", 10).unwrap();

            let id: u32 = ctx.eval("new_id()").unwrap();
            assert_eq!(id, 11);
            let id: u32 = ctx.eval("new_id()").unwrap();
            assert_eq!(id, 12);
            let id: u32 = ctx.eval("new_id()").unwrap();
            assert_eq!(id, 13);
        })
    }

    #[test]
    fn call_js_fn_with_var_args() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        let res: Vec<i8> = ctx.with(|ctx| {
            let func: Function = ctx
                .eval(
                    r#"
                  (...x) => [x.length, ...x]
                "#,
                )
                .unwrap();
            func.call((Args(vec![1, 2, 3]),)).unwrap()
        });
        assert_eq!(res.len(), 4);
        assert_eq!(res[0], 3);
        assert_eq!(res[1], 1);
        assert_eq!(res[2], 2);
        assert_eq!(res[3], 3);
    }

    #[test]
    fn call_js_fn_with_rest_args() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        let res: Vec<i8> = ctx.with(|ctx| {
            let func: Function = ctx
                .eval(
                    r#"
                  (a, b, ...x) => [a, b, x.length, ...x]
                "#,
                )
                .unwrap();
            func.call((-2, -1, Args(vec![1, 2]))).unwrap()
        });
        assert_eq!(res.len(), 5);
        assert_eq!(res[0], -2);
        assert_eq!(res[1], -1);
        assert_eq!(res[2], 2);
        assert_eq!(res[3], 1);
        assert_eq!(res[4], 2);
    }

    #[test]
    fn call_rust_fn_with_var_args() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        let res: Vec<i8> = ctx.with(|ctx| {
            let func = Function::new(ctx, "test_fn", |args: Args<i8>| {
                use std::iter::once;
                once(args.len() as i8)
                    .chain(args.iter().cloned())
                    .collect::<Vec<_>>()
            })
            .unwrap();
            ctx.globals().set("test_fn", func).unwrap();
            ctx.eval(
                r#"
                  test_fn(1, 2, 3)
                "#,
            )
            .unwrap()
        });
        assert_eq!(res.len(), 4);
        assert_eq!(res[0], 3);
        assert_eq!(res[1], 1);
        assert_eq!(res[2], 2);
        assert_eq!(res[3], 3);
    }

    #[test]
    fn call_rust_fn_with_rest_args() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        let res: Vec<i8> = ctx.with(|ctx| {
            let func = Function::new(ctx, "test_fn", |arg1: i8, arg2: i8, args: Args<i8>| {
                use std::iter::once;
                once(arg1)
                    .chain(once(arg2))
                    .chain(once(args.len() as i8))
                    .chain(args.iter().cloned())
                    .collect::<Vec<_>>()
            })
            .unwrap();
            ctx.globals().set("test_fn", func).unwrap();
            ctx.eval(
                r#"
                  test_fn(-2, -1, 1, 2)
                "#,
            )
            .unwrap()
        });
        assert_eq!(res.len(), 5);
        assert_eq!(res[0], -2);
        assert_eq!(res[1], -1);
        assert_eq!(res[2], 2);
        assert_eq!(res[3], 1);
        assert_eq!(res[4], 2);
    }
}
