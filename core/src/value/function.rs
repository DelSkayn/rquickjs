//! JS functions and rust callbacks.

use crate::{qjs, Ctx, Error, FromJs, IntoAtom, IntoJs, Object, ParallelSend, Result, Value};

mod args;
mod as_args;
mod as_func;
mod ffi;
mod types;

use args::{FromInput, Input};
pub use as_args::{AsArguments, CallInput, IntoInput};
pub use as_func::AsFunction;
use ffi::JsFunction;
pub use types::{Func, Method, MutFn, OnceFn, Opt, Rest, This};

#[cfg(feature = "futures")]
pub use types::Async;

/// Rust representation of a javascript function.
#[derive(Debug, Clone, PartialEq)]
pub struct Function<'js>(pub(crate) Value<'js>);

impl<'js> Function<'js> {
    pub fn new<F, A, R>(ctx: Ctx<'js>, func: F) -> Result<Self>
    where
        F: AsFunction<'js, A, R> + ParallelSend + 'js,
    {
        let func = JsFunction::new(move |input: &Input<'js>| func.call(input));
        let func = unsafe {
            let func = func.into_js_value(ctx);
            Self::from_js_value(ctx, func)
        };
        F::post(ctx, &func)?;
        func.set_length(F::num_args().start)?;
        Ok(func)
    }

    /// Set the `length` property
    pub fn set_length(&self, len: usize) -> Result<()> {
        let ctx = self.0.ctx;
        let func = self.0.as_js_value();
        let atom = "length".into_atom(ctx)?;
        let len = len.into_js(ctx)?;

        unsafe {
            let res = qjs::JS_DefinePropertyValue(
                ctx.as_ptr(),
                func,
                atom.atom,
                len.into_js_value(),
                (qjs::JS_PROP_CONFIGURABLE | qjs::JS_PROP_THROW) as _,
            );
            if res < 0 {
                return Err(self.ctx.raise_exception());
            }
        };

        Ok(())
    }

    /// Set the `name` property
    pub fn set_name<S: AsRef<str>>(&self, name: S) -> Result<()> {
        let ctx = self.0.ctx;
        let func = self.0.as_js_value();
        let name_atom = "name".into_atom(ctx)?;
        let name = name.as_ref().into_js(ctx)?;

        unsafe {
            let res = qjs::JS_DefinePropertyValue(
                ctx.as_ptr(),
                func,
                name_atom.atom,
                name.into_js_value(),
                (qjs::JS_PROP_CONFIGURABLE | qjs::JS_PROP_THROW) as _,
            );
            if res < 0 {
                return Err(ctx.raise_exception());
            }
        };

        Ok(())
    }

    /// Call a function with given arguments
    ///
    /// You can use tuples to pass arguments. The `()` treated as no arguments, the `(arg,)` as a single argument and so on.
    ///
    /// To call function on a given `this` you can pass `This(this)` as a first argument.
    /// By default an `undefined` will be passed as `this`.
    pub fn call<A, R>(&self, args: A) -> Result<R>
    where
        A: AsArguments<'js>,
        R: FromJs<'js>,
    {
        args.apply(self)
    }

    /// Immadiate call of function
    pub(crate) fn call_raw(&self, input: &CallInput) -> Result<Value<'js>> {
        let ctx = self.0.ctx;
        Ok(unsafe {
            let val = qjs::JS_Call(
                ctx.as_ptr(),
                self.0.as_js_value(),
                input.this,
                input.args.len() as _,
                input.args.as_ptr() as _,
            );
            let val = ctx.handle_exception(val)?;
            Value::from_js_value(ctx, val)
        })
    }

    /// Call a constructor with given arguments
    ///
    /// You can use tuples to pass arguments. The `()` treated as no arguments, the `(arg,)` as a single argument and so on.
    ///
    /// To call constructor on a given `this` you can pass `This(this)` as a first argument.
    pub fn construct<A, R>(&self, args: A) -> Result<R>
    where
        A: AsArguments<'js>,
        R: FromJs<'js>,
    {
        args.construct(self)
    }

    /// Immadiate call of function as a constructor
    pub(crate) fn construct_raw(&self, input: &CallInput) -> Result<Value<'js>> {
        let ctx = self.0.ctx;
        Ok(unsafe {
            let val = if input.has_this() {
                qjs::JS_CallConstructor2(
                    ctx.as_ptr(),
                    self.0.as_js_value(),
                    input.this,
                    input.args.len() as _,
                    input.args.as_ptr() as _,
                )
            } else {
                qjs::JS_CallConstructor(
                    ctx.as_ptr(),
                    self.0.as_js_value(),
                    input.args.len() as _,
                    input.args.as_ptr() as _,
                )
            };
            let val = ctx.handle_exception(val)?;
            Value::from_js_value(ctx, val)
        })
    }

    /// Deferred call a function with given arguments
    ///
    /// You can use tuples to pass arguments. The `()` treated as no arguments, the `(arg,)` as a single argument and so on.
    ///
    /// To call function on a given `this` you can pass `This(this)` as a first argument.
    /// By default an `undefined` will be passed as `this`.
    pub fn defer_call<A>(&self, args: A) -> Result<()>
    where
        A: AsArguments<'js>,
    {
        args.defer_apply(self)
    }

    /// Deferred call of function
    pub(crate) fn defer_call_raw(&self, input: &mut CallInput<'js>) -> Result<()> {
        let ctx = self.0.ctx;
        input.this_arg();
        input.arg(self.clone())?;
        unsafe {
            if qjs::JS_EnqueueJob(
                ctx.as_ptr(),
                Some(Self::defer_call_job),
                input.args.len() as _,
                input.args.as_ptr() as _,
            ) < 0
            {
                return Err(ctx.raise_exception());
            }
        }
        Ok(())
    }

    unsafe extern "C" fn defer_call_job(
        ctx: *mut qjs::JSContext,
        argc: qjs::c_int,
        argv: *mut qjs::JSValue,
    ) -> qjs::JSValue {
        let func = *argv.offset((argc - 1) as _);
        let this = *argv.offset((argc - 2) as _);
        let argc = argc - 2;
        qjs::JS_Call(ctx, func, this, argc, argv)
    }

    /// Check that function is a constructor
    pub fn is_constructor(&self) -> bool {
        0 != unsafe { qjs::JS_IsConstructor(self.0.ctx.as_ptr(), self.0.as_js_value()) }
    }

    /// Mark the function as a constructor
    pub fn set_constructor(&self, flag: bool) {
        unsafe {
            qjs::JS_SetConstructorBit(self.0.ctx.as_ptr(), self.0.as_js_value(), i32::from(flag))
        };
    }

    /// Set a function prototype
    ///
    /// Actually this method does the following:
    /// ```js
    /// func.prototype = proto;
    /// proto.constructor = func;
    /// ```
    pub fn set_prototype(&self, proto: &Object<'js>) {
        unsafe {
            qjs::JS_SetConstructor(
                self.0.ctx.as_ptr(),
                self.0.as_js_value(),
                proto.0.as_js_value(),
            )
        };
    }

    /// Get a function prototype
    ///
    /// Actually this method returns the `func.prototype`.
    pub fn get_prototype(&self) -> Result<Object<'js>> {
        let ctx = self.0.ctx;
        let value = self.0.as_js_value();
        Ok(unsafe {
            //TODO: can a function not have a prototype?.
            let proto = ctx.handle_exception(qjs::JS_GetPropertyStr(
                ctx.as_ptr(),
                value,
                "prototype\0".as_ptr() as _,
            ))?;
            if qjs::JS_IsObject(proto) {
                Object::from_js_value(ctx, proto)
            } else {
                return Err(Error::Unknown);
            }
        })
    }

    /// Reference as an object
    #[inline]
    pub fn as_object(&self) -> &Object<'js> {
        unsafe { &*(self as *const _ as *const Object) }
    }

    /// Convert into an object
    #[inline]
    pub fn into_object(self) -> Object<'js> {
        Object(self.0)
    }

    /// Convert from an object
    pub fn from_object(object: Object<'js>) -> Result<Self> {
        if object.is_function() {
            Ok(Self(object.0))
        } else {
            Err(Error::new_from_js("object", "function"))
        }
    }

    pub(crate) unsafe fn init_raw(rt: *mut qjs::JSRuntime) {
        JsFunction::register(rt);
    }
}

#[cfg(test)]
mod test {
    use crate::{prelude::*, *};
    use approx::assert_abs_diff_eq as assert_approx_eq;

    #[test]
    fn call_js_fn_with_no_args_and_no_return() {
        test_with(|ctx| {
            let f: Function = ctx.eval("() => {}").unwrap();

            let _: () = ().apply(&f).unwrap();
            let _: () = f.call(()).unwrap();
        })
    }

    #[test]
    fn call_js_fn_with_no_args_and_return() {
        test_with(|ctx| {
            let f: Function = ctx.eval("() => 42").unwrap();

            let res: i32 = ().apply(&f).unwrap();
            assert_eq!(res, 42);

            let res: i32 = f.call(()).unwrap();
            assert_eq!(res, 42);
        })
    }

    #[test]
    fn call_js_fn_with_1_arg_and_return() {
        test_with(|ctx| {
            let f: Function = ctx.eval("a => a + 4").unwrap();

            let res: i32 = (3,).apply(&f).unwrap();
            assert_eq!(res, 7);

            let res: i32 = f.call((1,)).unwrap();
            assert_eq!(res, 5);
        })
    }

    #[test]
    fn call_js_fn_with_2_args_and_return() {
        test_with(|ctx| {
            let f: Function = ctx.eval("(a, b) => a * b + 4").unwrap();

            let res: i32 = (3, 4).apply(&f).unwrap();
            assert_eq!(res, 16);

            let res: i32 = f.call((5, 1)).unwrap();
            assert_eq!(res, 9);
        })
    }

    #[test]
    fn call_js_fn_with_var_args_and_return() {
        let res: Vec<i8> = test_with(|ctx| {
            let func: Function = ctx
                .eval(
                    r#"
                  (...x) => [x.length, ...x]
                "#,
                )
                .unwrap();
            func.call((Rest(vec![1, 2, 3]),)).unwrap()
        });
        assert_eq!(res.len(), 4);
        assert_eq!(res[0], 3);
        assert_eq!(res[1], 1);
        assert_eq!(res[2], 2);
        assert_eq!(res[3], 3);
    }

    #[test]
    fn call_js_fn_with_rest_args_and_return() {
        let res: Vec<i8> = test_with(|ctx| {
            let func: Function = ctx
                .eval(
                    r#"
                  (a, b, ...x) => [a, b, x.length, ...x]
                "#,
                )
                .unwrap();
            func.call((-2, -1, Rest(vec![1, 2]))).unwrap()
        });
        assert_eq!(res.len(), 5);
        assert_eq!(res[0], -2);
        assert_eq!(res[1], -1);
        assert_eq!(res[2], 2);
        assert_eq!(res[3], 1);
        assert_eq!(res[4], 2);
    }

    #[test]
    fn call_js_fn_with_no_args_and_throw() {
        test_with(|ctx| {
            let f: Function = ctx
                .eval("() => { throw new Error('unimplemented'); }")
                .unwrap();

            if let Err(Error::Exception) = f.call::<_, ()>(()) {
                let exception = Exception::from_js(ctx, ctx.catch()).unwrap();
                assert_eq!(exception.message().as_deref(), Some("unimplemented"));
            } else {
                panic!("Should throws");
            }
        })
    }

    #[test]
    fn call_js_fn_with_this_and_no_args_and_return() {
        test_with(|ctx| {
            let f: Function = ctx.eval("function f() { return this.val; } f").unwrap();
            let obj = Object::new(ctx).unwrap();
            obj.set("val", 42).unwrap();

            let res: i32 = (This(obj.clone()),).apply(&f).unwrap();
            assert_eq!(res, 42);
            let res: i32 = f.call((This(obj),)).unwrap();
            assert_eq!(res, 42);
        })
    }

    #[test]
    fn call_js_fn_with_this_and_1_arg_and_return() {
        test_with(|ctx| {
            let f: Function = ctx
                .eval("function f(a) { return this.val * a; } f")
                .unwrap();
            let obj = Object::new(ctx).unwrap();
            obj.set("val", 3).unwrap();

            let res: i32 = (This(obj.clone()), 2).apply(&f).unwrap();
            assert_eq!(res, 6);
            let res: i32 = f.call((This(obj), 3)).unwrap();
            assert_eq!(res, 9);
        })
    }

    #[test]
    fn call_js_fn_with_1_arg_deferred() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        assert!(!rt.is_job_pending());
        ctx.with(|ctx| {
            let g = ctx.globals();
            let f: Function = ctx.eval("(obj) => { obj.called = true; }").unwrap();
            f.defer_call((g.clone(),)).unwrap();
            let c: Value = g.get("called").unwrap();
            assert_eq!(c.type_of(), Type::Undefined);
        });
        assert!(rt.is_job_pending());
        rt.execute_pending_job().unwrap();
        ctx.with(|ctx| {
            let g = ctx.globals();
            let c: Value = g.get("called").unwrap();
            assert_eq!(c.type_of(), Type::Bool);
        });
    }

    fn test() {
        println!("test");
    }

    #[test]
    fn static_callback() {
        test_with(|ctx| {
            let f = Function::new(ctx, test).unwrap();
            f.set_name("test").unwrap();
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
        test_with(|ctx| {
            #[allow(clippy::mutex_atomic)]
            let called = Arc::new(Mutex::new(false));
            let called_clone = called.clone();
            let f = Function::new(ctx, move || {
                (*called_clone.lock().unwrap()) = true;
            })
            .unwrap();
            f.set_name("test").unwrap();

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
        test_with(|ctx| {
            let mut v = 0;
            let f = Function::new(
                ctx,
                MutFn::from(move || {
                    v += 1;
                    v
                }),
            )
            .unwrap();
            f.set_name("test").unwrap();

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
    fn recursively_called_mutable_callback() {
        test_with(|ctx| {
            let mut v = 0;
            let f = Function::new(
                ctx,
                MutFn::from(move |ctx: Ctx| {
                    v += 1;
                    ctx.globals()
                        .get::<_, Function>("foo")
                        .unwrap()
                        .call::<_, ()>(())
                        .unwrap();
                    v
                }),
            )
            .unwrap();
            ctx.globals().set("foo", f.clone()).unwrap();
            f.call::<_, ()>(()).unwrap();
        })
    }

    #[test]
    #[should_panic(
        expected = "Once function callback is already was used! Could it have been called twice?"
    )]
    fn repeatedly_called_once_callback() {
        test_with(|ctx| {
            let mut v = 0;
            let f = Function::new(
                ctx,
                OnceFn::from(move || {
                    v += 1;
                    v
                }),
            )
            .unwrap();
            ctx.globals().set("foo", f.clone()).unwrap();
            f.call::<_, ()>(()).unwrap();
            f.call::<_, ()>(()).unwrap();
        })
    }

    #[test]
    fn multiple_const_callbacks() {
        test_with(|ctx| {
            let globals = ctx.globals();
            globals.set("one", Func::new("one", || 1f64)).unwrap();
            globals.set("neg", Func::new("neg", |a: f64| -a)).unwrap();
            globals
                .set("add", Func::new("add", |a: f64, b: f64| a + b))
                .unwrap();

            let r: f64 = ctx.eval("neg(add(one(), 2))").unwrap();
            assert_approx_eq!(r, -3.0);
        })
    }

    #[test]
    fn mutable_callback_which_can_fail() {
        test_with(|ctx| {
            let globals = ctx.globals();
            let mut id_alloc = 0;
            globals
                .set(
                    "new_id",
                    Func::from(MutFn::from(move || {
                        id_alloc += 1;
                        if id_alloc < 4 {
                            Ok(id_alloc)
                        } else {
                            Err(Error::Unknown)
                        }
                    })),
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
        test_with(|ctx| {
            let globals = ctx.globals();
            let mut id_alloc = 0;
            globals
                .set(
                    "new_id",
                    Func::from(MutFn::from(move |ctx: Ctx| {
                        let initial: Option<u32> = ctx.globals().get("initial_id")?;
                        if let Some(initial) = initial {
                            id_alloc += 1;
                            Ok(id_alloc + initial)
                        } else {
                            Err(Error::Unknown)
                        }
                    })),
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
    fn call_rust_fn_with_ctx_and_value() {
        test_with(|ctx| {
            let func = Func::from(|ctx, val| {
                struct Args<'js>(Ctx<'js>, Value<'js>);
                let Args(ctx, val) = Args(ctx, val);
                ctx.globals().set("test_str", val).unwrap();
            });
            ctx.globals().set("test_fn", func).unwrap();
            ctx.eval::<(), _>(
                r#"
                  test_fn("test_str")
                "#,
            )
            .unwrap();
            let val: StdString = ctx.globals().get("test_str").unwrap();
            assert_eq!(val, "test_str");
        });
    }

    #[test]
    fn call_overloaded_callback() {
        test_with(|ctx| {
            let globals = ctx.globals();
            globals
                .set(
                    "calc",
                    Func::from((|a: f64, b: f64| (a + 1f64) * b, |a: f64| a + 1f64, || 1f64)),
                )
                .unwrap();

            let r: f64 = ctx.eval("calc()").unwrap();
            assert_approx_eq!(r, 1.0);
            let r: f64 = ctx.eval("calc(2)").unwrap();
            assert_approx_eq!(r, 3.0);
            let r: f64 = ctx.eval("calc(2, 3)").unwrap();
            assert_approx_eq!(r, 9.0);
        })
    }

    #[test]
    fn call_rust_fn_with_this_and_args() {
        let res: f64 = test_with(|ctx| {
            let func = Function::new(ctx, |this: This<Object>, a: f64, b: f64| {
                let x: f64 = this.get("x").unwrap();
                let y: f64 = this.get("y").unwrap();
                this.set("r", a * x + b * y).unwrap();
            })
            .unwrap();
            ctx.globals().set("test_fn", func).unwrap();
            ctx.eval(
                r#"
                  let test_obj = { x: 1, y: 2 };
                  test_fn.call(test_obj, 3, 4);
                  test_obj.r
                "#,
            )
            .unwrap()
        });
        assert_eq!(res, 11.0);
    }

    #[test]
    fn apply_rust_fn_with_this_and_args() {
        let res: f32 = test_with(|ctx| {
            let func = Function::new(ctx, |this: This<Object>, x: f32, y: f32| {
                let a: f32 = this.get("a").unwrap();
                let b: f32 = this.get("b").unwrap();
                a * x + b * y
            })
            .unwrap();
            ctx.globals().set("test_fn", func).unwrap();
            ctx.eval(
                r#"
                  let test_obj = { a: 1, b: 2 };
                  test_fn.apply(test_obj, [3, 4])
                "#,
            )
            .unwrap()
        });
        assert_eq!(res, 11.0);
    }

    #[test]
    fn bind_rust_fn_with_this_and_call_with_args() {
        let res: f32 = test_with(|ctx| {
            let func = Function::new(ctx, |this: This<Object>, x: f32, y: f32| {
                let a: f32 = this.get("a").unwrap();
                let b: f32 = this.get("b").unwrap();
                a * x + b * y
            })
            .unwrap();
            ctx.globals().set("test_fn", func).unwrap();
            ctx.eval(
                r#"
                  let test_obj = { a: 1, b: 2 };
                  test_fn.bind(test_obj)(3, 4)
                "#,
            )
            .unwrap()
        });
        assert_eq!(res, 11.0);
    }

    #[test]
    fn call_rust_fn_with_var_args() {
        let res: Vec<i8> = test_with(|ctx| {
            let func = Function::new(ctx, |args: Rest<i8>| {
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
        let res: Vec<i8> = test_with(|ctx| {
            let func = Function::new(ctx, |arg1: i8, arg2: i8, args: Rest<i8>| {
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

    #[test]
    fn js_fn_wrappers() {
        test_with(|ctx| {
            let global = ctx.globals();
            global
                .set(
                    "cat",
                    Func::from(|a: StdString, b: StdString| format!("{a}{b}")),
                )
                .unwrap();
            let res: StdString = ctx.eval("cat(\"foo\", \"bar\")").unwrap();
            assert_eq!(res, "foobar");

            let mut log = Vec::<StdString>::new();
            global
                .set(
                    "log",
                    Func::from(MutFn::from(move |msg: StdString| {
                        log.push(msg);
                        log.len() as u32
                    })),
                )
                .unwrap();
            let n: u32 = ctx.eval("log(\"foo\") + log(\"bar\")").unwrap();
            assert_eq!(n, 3);
        });
    }
}
