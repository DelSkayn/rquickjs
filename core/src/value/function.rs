//! JavaScript function functionality

use crate::{
    atom::PredefinedAtom,
    class::{Class, JsClass},
    function::ffi::RustFunc,
    qjs, Ctx, Error, FromJs, IntoJs, Object, Result, Value,
};

mod args;
mod ffi;
mod into_func;
mod params;
mod types;

pub use args::{Args, IntoArg, IntoArgs};
pub use ffi::RustFunction;
pub use params::{FromParam, FromParams, ParamRequirement, Params, ParamsAccessor};
#[cfg(feature = "futures")]
pub use types::Async;
pub use types::{Exhaustive, Flat, Func, FuncArg, MutFn, Null, OnceFn, Opt, Rest, This};

/// A trait for converting a Rust function to a JavaScript function.
pub trait IntoJsFunc<'js, P> {
    /// Returns the requirements this function has for the set of arguments used to call this
    /// function.
    fn param_requirements() -> ParamRequirement;

    /// Call the function with the given parameters.
    fn call<'a>(&self, params: Params<'a, 'js>) -> Result<Value<'js>>;
}

/// A trait for functions callable from JavaScript but static,
/// Used for implementing callable objects.
pub trait StaticJsFunction {
    fn call<'a, 'js>(params: Params<'a, 'js>) -> Result<Value<'js>>;
}

/// A JavaScript function.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Function<'js>(pub(crate) Object<'js>);

impl<'js> Function<'js> {
    /// Create a new function from a Rust function which implements [`IntoJsFunc`].
    pub fn new<P, F>(ctx: Ctx<'js>, f: F) -> Result<Self>
    where
        F: IntoJsFunc<'js, P> + 'js,
    {
        let func = Box::new(move |params: Params<'_, 'js>| {
            params.check_params(F::param_requirements())?;
            f.call(params)
        }) as Box<dyn RustFunc<'js> + 'js>;

        let cls = Class::instance(ctx, RustFunction(func))?;
        debug_assert!(cls.is_function());
        Function(cls.into_inner()).with_length(F::param_requirements().min())
    }

    /// Call the function with given arguments.
    pub fn call<A, R>(&self, args: A) -> Result<R>
    where
        A: IntoArgs<'js>,
        R: FromJs<'js>,
    {
        let ctx = self.ctx();
        let num = args.num_args();
        let mut accum_args = Args::new(ctx.clone(), num);
        args.into_args(&mut accum_args)?;
        self.call_arg(accum_args)
    }

    /// Call the function with given arguments in the form of an [`Args`] object.
    pub fn call_arg<R>(&self, args: Args<'js>) -> Result<R>
    where
        R: FromJs<'js>,
    {
        args.apply(self)
    }

    /// Defer call the function with given arguments.
    ///
    /// Calling a function with defer is equivalent to calling a JavaScript function with
    /// `setTimeout(func,0)`.
    pub fn defer<A>(&self, args: A) -> Result<()>
    where
        A: IntoArgs<'js>,
    {
        let ctx = self.ctx();
        let num = args.num_args();
        let mut accum_args = Args::new(ctx.clone(), num);
        args.into_args(&mut accum_args)?;
        self.defer_arg(accum_args)?;
        Ok(())
    }

    /// Defer a function call with given arguments.
    pub fn defer_arg(&self, args: Args<'js>) -> Result<()> {
        args.defer(self.clone())
    }

    /// Set the `name` property of this function
    pub fn set_name<S: AsRef<str>>(&self, name: S) -> Result<()> {
        let name = name.as_ref().into_js(self.ctx())?;
        unsafe {
            let res = qjs::JS_DefinePropertyValue(
                self.0.ctx.as_ptr(),
                self.0.as_js_value(),
                PredefinedAtom::Name as qjs::JSAtom,
                name.into_js_value(),
                (qjs::JS_PROP_CONFIGURABLE | qjs::JS_PROP_THROW) as _,
            );
            if res < 0 {
                return Err(self.0.ctx.raise_exception());
            }
        };
        Ok(())
    }

    /// Set the `name` property of this function and then return self.
    pub fn with_name<S: AsRef<str>>(self, name: S) -> Result<Self> {
        self.set_name(name)?;
        Ok(self)
    }

    /// Sets the `length` property of the function.
    pub fn set_length(&self, len: usize) -> Result<()> {
        let len = len.into_js(self.ctx())?;
        unsafe {
            let res = qjs::JS_DefinePropertyValue(
                self.0.ctx.as_ptr(),
                self.0.as_js_value(),
                PredefinedAtom::Length as qjs::JSAtom,
                len.into_js_value(),
                (qjs::JS_PROP_CONFIGURABLE | qjs::JS_PROP_THROW) as _,
            );
            if res < 0 {
                return Err(self.0.ctx.raise_exception());
            }
        };
        Ok(())
    }

    /// Sets the `length` property of the function and return self.
    pub fn with_length(self, len: usize) -> Result<Self> {
        self.set_length(len)?;
        Ok(self)
    }

    /// Returns the prototype which all JavaScript function by default have as its prototype, i.e.
    /// `Function.prototype`.
    pub fn prototype(ctx: Ctx<'js>) -> Object<'js> {
        let res = unsafe {
            let v = qjs::JS_DupValue(qjs::JS_GetFunctionProto(ctx.as_ptr()));
            Value::from_js_value(ctx, v)
        };
        // as far is I know this should always be an object.
        res.into_object()
            .expect("`Function.prototype` wasn't an object")
    }

    /// Returns whether this function is an constructor.
    pub fn is_constructor(&self) -> bool {
        let res = unsafe { qjs::JS_IsConstructor(self.ctx().as_ptr(), self.0.as_js_value()) };
        res != 0
    }

    /// Set whether this function is a constructor or not.
    pub fn set_constructor(&self, is_constructor: bool) {
        unsafe {
            qjs::JS_SetConstructorBit(
                self.ctx().as_ptr(),
                self.0.as_js_value(),
                is_constructor as i32,
            )
        };
    }

    /// Set whether this function is a constructor or not then return self.
    pub fn with_constructor(self, is_constructor: bool) -> Self {
        self.set_constructor(is_constructor);
        self
    }
}

/// A function which can be used as a constructor.
///
/// Is a subtype of function.
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct Constructor<'js>(pub(crate) Function<'js>);

impl<'js> Constructor<'js> {
    /// Creates a Rust constructor function for a Rust class.
    ///
    /// Note that this function creates a constructor from a given function, the returned constructor
    /// is thus not the same as the one returned from [`JsClass::constructor`].
    pub fn new_class<C, F, P>(ctx: Ctx<'js>, f: F) -> Result<Self>
    where
        F: IntoJsFunc<'js, P> + 'js,
        C: JsClass<'js>,
    {
        let func = Box::new(move |params: Params<'_, 'js>| -> Result<Value<'js>> {
            params.check_params(F::param_requirements())?;
            let this = params.this();
            let ctx = params.ctx().clone();

            // get the prototype of thie class from itself or the inate class prototype.
            let proto = this
                .into_function()
                .map(|func| func.get(PredefinedAtom::Prototype))
                .unwrap_or_else(|| Class::<C>::prototype(&ctx))?;

            let res = f.call(params)?;
            res.as_object()
                .ok_or_else(|| Error::IntoJs {
                    from: res.type_of().as_str(),
                    to: "object",
                    message: Some("rust constructor function did not return a object".to_owned()),
                })?
                .set_prototype(proto.as_ref())?;
            Ok(res)
        });
        let func = Function(Class::instance(ctx.clone(), RustFunction(func))?.into_inner())
            .with_name(C::NAME)?
            .with_constructor(true);
        unsafe {
            qjs::JS_SetConstructor(
                ctx.as_ptr(),
                func.as_js_value(),
                Class::<C>::prototype(&ctx)?
                    .as_ref()
                    .map(|x| x.as_js_value())
                    .unwrap_or(qjs::JS_NULL),
            )
        };
        Ok(Constructor(func))
    }

    /// Create a new Rust constructor function with a given prototype.
    ///
    /// Useful if the function does not return a Rust class.
    pub fn new_prototype<F, P>(ctx: &Ctx<'js>, prototype: Object<'js>, f: F) -> Result<Self>
    where
        F: IntoJsFunc<'js, P> + 'js,
    {
        let proto_clone = prototype.clone();
        let func = Box::new(move |params: Params<'_, 'js>| -> Result<Value<'js>> {
            params.check_params(F::param_requirements())?;
            let this = params.this();
            let proto = this
                .as_function()
                .map(|func| func.get(PredefinedAtom::Prototype))
                .unwrap_or_else(|| Ok(Some(proto_clone.clone())))?;

            let res = f.call(params)?;
            res.as_object()
                .ok_or_else(|| Error::IntoJs {
                    from: res.type_of().as_str(),
                    to: "object",
                    message: Some("rust constructor function did not return a object".to_owned()),
                })?
                .set_prototype(proto.as_ref())?;
            Ok(res)
        });
        let func = Function(Class::instance(ctx.clone(), RustFunction(func))?.into_inner())
            .with_constructor(true);
        unsafe {
            qjs::JS_SetConstructor(ctx.as_ptr(), func.as_js_value(), prototype.as_js_value())
        };
        Ok(Constructor(func))
    }

    /// Call the constructor as a constructor.
    ///
    /// Equivalent to calling any constructor function with the new keyword.
    pub fn construct<A, R>(&self, args: A) -> Result<R>
    where
        A: IntoArgs<'js>,
        R: FromJs<'js>,
    {
        let ctx = self.ctx();
        let num = args.num_args();
        let mut accum_args = Args::new(ctx.clone(), num);
        args.into_args(&mut accum_args)?;
        self.construct_args(accum_args)
    }

    /// Call the constructor as a constructor with an [`Args`] object.
    ///
    /// Equivalent to calling any constructor function with the new keyword.
    pub fn construct_args<R>(&self, args: Args<'js>) -> Result<R>
    where
        R: FromJs<'js>,
    {
        args.construct(self)
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
                let exception = Exception::from_js(&ctx, ctx.catch()).unwrap();
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
            f.defer((g.clone(),)).unwrap();
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
            let f = Function::new(ctx.clone(), test).unwrap();
            f.set_name("test").unwrap();
            let eval: Function = ctx.eval("a => { a() }").unwrap();
            (f.clone(),).apply::<()>(&eval).unwrap();
            f.call::<_, ()>(()).unwrap();

            let name: StdString = f.clone().into_inner().get("name").unwrap();
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
            let f = Function::new(ctx.clone(), move || {
                (*called_clone.lock().unwrap()) = true;
            })
            .unwrap();
            f.set_name("test").unwrap();

            let eval: Function = ctx.eval("a => { a() }").unwrap();
            eval.call::<_, ()>((f.clone(),)).unwrap();
            f.call::<_, ()>(()).unwrap();
            assert!(*called.lock().unwrap());

            let name: StdString = f.clone().into_inner().get("name").unwrap();
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
                ctx.clone(),
                MutFn::new(move || {
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

            let name: StdString = f.clone().into_inner().get("name").unwrap();
            assert_eq!(name, "test");

            let get_name: Function = ctx.eval("a => a.name").unwrap();
            let name: StdString = get_name.call((f.clone(),)).unwrap();
            assert_eq!(name, "test");
        })
    }

    #[test]
    #[should_panic(
        expected = "Error borrowing function: can't borrow a value as it is already borrowed"
    )]
    fn recursively_called_mutable_callback() {
        test_with(|ctx| {
            let mut v = 0;
            let f = Function::new(
                ctx.clone(),
                MutFn::new(move |ctx: Ctx| {
                    v += 1;
                    ctx.globals()
                        .get::<_, Function>("foo")
                        .unwrap()
                        .call::<_, ()>(())
                        .catch(&ctx)
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
        expected = "Error borrowing function: tried to use a value, which can only be used once, again."
    )]
    fn repeatedly_called_once_callback() {
        test_with(|ctx| {
            let mut v = 0;
            let f = Function::new(
                ctx.clone(),
                OnceFn::from(move || {
                    v += 1;
                    v
                }),
            )
            .unwrap();
            ctx.globals().set("foo", f.clone()).unwrap();
            f.call::<_, ()>(()).catch(&ctx).unwrap();
            f.call::<_, ()>(()).catch(&ctx).unwrap();
        })
    }

    #[test]
    fn multiple_const_callbacks() {
        test_with(|ctx| {
            let globals = ctx.globals();
            globals.set("one", Func::new(|| 1f64)).unwrap();
            globals.set("neg", Func::new(|a: f64| -a)).unwrap();
            globals
                .set("add", Func::new(|a: f64, b: f64| a + b))
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
    fn call_rust_fn_with_this_and_args() {
        let res: f64 = test_with(|ctx| {
            let func = Function::new(ctx.clone(), |this: This<Object>, a: f64, b: f64| {
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
            let func = Function::new(ctx.clone(), |this: This<Object>, x: f32, y: f32| {
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
            let func = Function::new(ctx.clone(), |this: This<Object>, x: f32, y: f32| {
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
            let func = Function::new(ctx.clone(), |args: Rest<i8>| {
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
            let func = Function::new(ctx.clone(), |arg1: i8, arg2: i8, args: Rest<i8>| {
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
