use crate::{
    qjs, value::rf::JsObjectRef, Ctx, FromJs, FromJsArgs, IntoAtom, IntoJs, IntoJsArgs, Object,
    Result, Runtime, SendWhenParallel, String, Value,
};
use std::{cell::RefCell, ffi::CString, mem, os::raw::c_int};

mod ffi;
use ffi::{FuncOpaque, FuncStatic};

mod as_js;
pub use as_js::{AsFunction, AsFunctionMut, This};

/// A trait which allows rquickjs to create a callback with only minimal overhead.
pub trait StaticFn<'js> {
    /// type of the this object that the function expects
    /// Generally the global object.
    type This: FromJs<'js>;
    /// type of the arguments that the function expects
    type Args: FromJsArgs<'js>;
    /// type of the return value
    type Result: IntoJs<'js>;

    /// Call the static function.
    fn call(ctx: Ctx<'js>, this: Self::This, args: Self::Args) -> Result<Self::Result>;
}

/// Implements StaticFn for a function
///
/// # Example
///
/// ```
/// # use rquickjs::{Runtime,Context,static_fn,Ctx, Function, Result};
/// # let rt = Runtime::new().unwrap();
/// # let context = Context::full(&rt).unwrap();
/// use rquickjs::function::StaticFn;
///
/// fn print<'js>(ctx: Ctx<'js>, _this: (), args: (String,)) -> Result<()>{
///     println!("{}",args.0);
///     Ok(())
/// }
///
/// static_fn!(print,FnPrint,(), (String,),());
///
/// context.with(|ctx|{
///     let print = Function::new_static::<FnPrint,_>(ctx, "print").unwrap();
///     ctx.globals().set("print", print);
///     // prints 'hello from javascript' to stdout
///     ctx.eval::<(),_>("print('hello from javascript!'); ");
/// });
/// ```
#[macro_export]
macro_rules! static_fn {
    ($func:ident, $name:ident, $this:ty, $args:ty,$res:ty) => {
        pub struct $name;

        impl<'js> StaticFn<'js> for $name {
            type This = $this;
            type Args = $args;
            type Result = $res;

            fn call(ctx: Ctx<'js>, this: Self::This, args: Self::Args) -> Result<Self::Result> {
                $func(ctx, this, args)
            }
        }
    };
}

/// The trait to convert Rust function to JS one
pub trait IntoFunction<'js, P> {
    /// Convert Rust function to JS
    fn into_function<N: AsRef<str>>(self, ctx: Ctx<'js>, name: N) -> Result<Function<'js>>;
}

impl<'js, F> IntoFunction<'js, ()> for F
where
    F: StaticFn<'js>,
{
    fn into_function<N: AsRef<str>>(self, ctx: Ctx<'js>, name: N) -> Result<Function<'js>> {
        Function::new_static::<F, _>(ctx, name)
    }
}

impl<'js, F, A, T, R> IntoFunction<'js, ((), A, T, R)> for F
where
    A: FromJsArgs<'js>,
    T: FromJs<'js>,
    R: IntoJs<'js>,
    F: Fn(Ctx<'js>, T, A) -> Result<R> + SendWhenParallel + 'static,
{
    fn into_function<N: AsRef<str>>(self, ctx: Ctx<'js>, name: N) -> Result<Function<'js>> {
        Function::new(ctx, name, self)
    }
}

impl<'js, F, A, T, R> IntoFunction<'js, ((), A, T, R, ())> for F
where
    A: FromJsArgs<'js>,
    T: FromJs<'js>,
    R: IntoJs<'js>,
    F: FnMut(Ctx<'js>, T, A) -> Result<R> + SendWhenParallel + 'static,
{
    fn into_function<N: AsRef<str>>(self, ctx: Ctx<'js>, name: N) -> Result<Function<'js>> {
        Function::new_mut(ctx, name, self)
    }
}

impl<'js, F, A, R> IntoFunction<'js, (A, R)> for F
where
    F: AsFunction<'js, A, R> + SendWhenParallel + 'static,
{
    fn into_function<N: AsRef<str>>(self, ctx: Ctx<'js>, name: N) -> Result<Function<'js>> {
        Function::new2(ctx, name, self)
    }
}

impl<'js, F, A, R> IntoFunction<'js, (A, R, ())> for F
where
    F: AsFunctionMut<'js, A, R> + SendWhenParallel + 'static,
{
    fn into_function<N: AsRef<str>>(self, ctx: Ctx<'js>, name: N) -> Result<Function<'js>> {
        Function::new_mut2(ctx, name, self)
    }
}

/// Rust representation of a javascript function.
#[derive(Debug, Clone, PartialEq)]
pub struct Function<'js>(pub(crate) JsObjectRef<'js>);

impl<'js> Function<'js> {
    /// Create a new static function.
    ///
    /// Static functions do not have any context unlike closures and must implement
    /// [`StaticFn`](function/trait.StaticFn.html).
    /// Static functions have minimal overhead compared to other functions but
    /// have more restrictions and require a type to 'carry' the function.
    ///
    /// # Example
    ///
    /// ```
    /// # use rquickjs::{Runtime,Context,static_fn,Ctx, Function, Result};
    /// # let rt = Runtime::new().unwrap();
    /// # let context = Context::full(&rt).unwrap();
    /// use rquickjs::function::StaticFn;
    ///
    /// fn print<'js>(ctx: Ctx<'js>, _this: (), args: (String,)) -> Result<()>{
    ///     println!("{}",args.0);
    ///     Ok(())
    /// }
    ///
    /// static_fn!(print,FnPrint,(), (String,),());
    ///
    /// context.with(|ctx|{
    ///     let print = Function::new_static::<FnPrint,_>(ctx, "print").unwrap();
    ///     ctx.globals().set("print", print);
    ///     // prints 'hello from javascript' to stdout
    ///     ctx.eval::<(),_>("print('hello from javascript!'); ");
    /// });
    /// ```
    pub fn new_static<F, N>(ctx: Ctx<'js>, name: N) -> Result<Self>
    where
        N: AsRef<str>,
        F: StaticFn<'js>,
    {
        let name = CString::new(name.as_ref())?;
        unsafe {
            let val = qjs::JS_NewCFunction2(
                ctx.ctx,
                Some(FuncStatic::<F>::call),
                name.as_ptr(),
                F::Args::LEN as c_int,
                qjs::JSCFunctionEnum_JS_CFUNC_generic,
                0,
            );
            Ok(Function(JsObjectRef::from_js_value(ctx, val)))
        }
    }

    pub fn new<F, A, T, R, N>(ctx: Ctx<'js>, name: N, func: F) -> Result<Self>
    where
        N: AsRef<str>,
        A: FromJsArgs<'js>,
        T: FromJs<'js>,
        R: IntoJs<'js>,
        F: Fn(Ctx<'js>, T, A) -> Result<R> + SendWhenParallel + 'static,
    {
        let opaque = FuncOpaque::new(move |ctx, this, args| {
            let this = T::from_js(ctx, this)?;
            let args = A::from_js_args(ctx, args)?;

            let res = func(ctx, this, args)?;
            res.into_js(ctx)
        });

        Self::new_raw(ctx, name, opaque)
    }

    pub fn new_mut<F, A, T, R, N>(ctx: Ctx<'js>, name: N, func: F) -> Result<Self>
    where
        N: AsRef<str>,
        A: FromJsArgs<'js>,
        T: FromJs<'js>,
        R: IntoJs<'js>,
        F: FnMut(Ctx<'js>, T, A) -> Result<R> + SendWhenParallel + 'static,
    {
        let func = RefCell::new(func);

        let opaque = FuncOpaque::new(move |ctx, this, args| {
            let this = T::from_js(ctx, this)?;
            let args = A::from_js_args(ctx, args)?;

            let mut func = func.try_borrow_mut()
                .expect("Mutable function callback is already in use! Could it have been called recursively?");

            let res = func(ctx, this, args)?;

            res.into_js(ctx)
        });

        Self::new_raw(ctx, name, opaque)
    }

    pub fn new2<F, A, R, N>(ctx: Ctx<'js>, name: N, func: F) -> Result<Self>
    where
        N: AsRef<str>,
        F: AsFunction<'js, A, R> + SendWhenParallel + 'static,
    {
        let opaque = FuncOpaque::new(move |ctx, this, args| func.call(ctx, this, args));

        Self::new_raw(ctx, name, opaque)
    }

    pub fn new_mut2<F, A, R, N>(ctx: Ctx<'js>, name: N, func: F) -> Result<Self>
    where
        N: AsRef<str>,
        F: AsFunctionMut<'js, A, R> + SendWhenParallel + 'static,
    {
        let func = RefCell::new(func);

        let opaque = FuncOpaque::new(move |ctx, this, args| {
            let mut func = func.try_borrow_mut()
                .expect("Mutable function callback is already in use! Could it have been called recursively?");

            func.call(ctx, this, args)
        });

        Self::new_raw(ctx, name, opaque)
    }

    fn new_raw<N: AsRef<str>>(ctx: Ctx<'js>, name: N, opaque: FuncOpaque) -> Result<Self> {
        let obj = unsafe { opaque.to_js_value(ctx) };
        let name_field = "name".into_atom(ctx);
        let name_value = String::from_str(ctx, name.as_ref())?;
        unsafe { qjs::JS_SetProperty(ctx.ctx, obj, name_field.atom, name_value.0.into_js_value()) };
        Ok(Function(unsafe { JsObjectRef::from_js_value(ctx, obj) }))
    }

    /// Call a function with given arguments with the `this` as the global context object.
    pub fn call<A, R>(&self, args: A) -> Result<R>
    where
        A: IntoJsArgs<'js>,
        R: FromJs<'js>,
    {
        self.call_on(self.0.ctx.globals(), args)
    }

    /// Call a function with given arguments on a given `this`
    pub fn call_on<A, T, R>(&self, this: T, args: A) -> Result<R>
    where
        A: IntoJsArgs<'js>,
        R: FromJs<'js>,
        T: IntoJs<'js>,
    {
        let args = args.into_js_args(self.0.ctx)?;
        let this = this.into_js(self.0.ctx)?;
        let len = args.len();
        let res = unsafe {
            // Dont drop args value
            let mut args: Vec<_> = args.iter().map(|x| x.as_js_value()).collect();
            let val = qjs::JS_Call(
                self.0.ctx.ctx,
                self.0.as_js_value(),
                this.as_js_value(),
                len as i32,
                args.as_mut_ptr(),
            );
            R::from_js(self.0.ctx, Value::from_js_value(self.0.ctx, val)?)
        };
        // Make sure the lifetime of args remains valid during the
        // entire duration of the call.
        mem::drop(args);
        mem::drop(this);
        res
    }

    pub fn into_object(self) -> Object<'js> {
        Object(self.0)
    }

    pub(crate) fn new_class(rt: &Runtime) -> qjs::JSClassID {
        let rt_ref = rt.inner.lock();
        unsafe { FuncOpaque::new_fn_class(rt_ref.rt) }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::*;
    #[test]
    fn js_call() {
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

    fn test<'js>(_: Ctx<'js>, _: (), _: ()) -> Result<()> {
        println!("test");
        Ok(())
    }

    static_fn!(test, Test, (), (), ());

    #[test]
    fn static_callback() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            //let f = Function::new_static::<Test, _>(ctx, "test").unwrap();
            let f = Test.into_function(ctx, "test").unwrap();
            let eval: Function = ctx.eval("a => { a() }").unwrap();
            eval.call::<_, ()>(f.clone()).unwrap();
            f.call::<_, ()>(()).unwrap();

            let name: StdString = f.clone().into_object().get("name").unwrap();
            assert_eq!(name, "test");

            let get_name: Function = ctx.eval("a => a.name").unwrap();
            let name: StdString = get_name.call(f.clone()).unwrap();
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
            let f = Function::new(ctx, "test", move |_, _: (), _: ()| {
                (*called_clone.lock().unwrap()) = true;
                Ok(())
            })
            .unwrap();

            let eval: Function = ctx.eval("a => { a() }").unwrap();
            eval.call::<_, ()>(f.clone()).unwrap();
            f.call::<_, ()>(()).unwrap();
            assert!(*called.lock().unwrap());

            let name: StdString = f.clone().into_object().get("name").unwrap();
            assert_eq!(name, "test");

            let get_name: Function = ctx.eval("a => a.name").unwrap();
            let name: StdString = get_name.call(f.clone()).unwrap();
            assert_eq!(name, "test");
        })
    }

    #[test]
    fn mut_callback() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let mut v = 0;
            let f = Function::new_mut(ctx, "test", move |_, _: (), _: ()| {
                v += 1;
                return Ok(v);
            })
            .unwrap();

            let eval: Function = ctx.eval("a => a()").unwrap();
            assert_eq!(eval.call::<_, i32>(f.clone()).unwrap(), 1);
            assert_eq!(eval.call::<_, i32>(f.clone()).unwrap(), 2);
            assert_eq!(eval.call::<_, i32>(f.clone()).unwrap(), 3);

            let name: StdString = f.clone().into_object().get("name").unwrap();
            assert_eq!(name, "test");

            let get_name: Function = ctx.eval("a => a.name").unwrap();
            let name: StdString = get_name.call(f.clone()).unwrap();
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
            let f = Function::new_mut(ctx, "test", move |ctx, _: (), _: ()| {
                v += 1;
                ctx.globals()
                    .get::<_, Function>("foo")
                    .unwrap()
                    .call::<_, ()>(())
                    .unwrap();
                return Ok(v);
            })
            .unwrap();
            ctx.globals().set("foo", f.clone()).unwrap();
            f.call::<_, ()>(()).unwrap();
        })
    }

    #[test]
    fn const_callback2() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let globals = ctx.globals();
            globals
                .set("one", Function::new2(ctx, "id", || 1f64).unwrap())
                .unwrap();
            globals
                .set("neg", Function::new2(ctx, "neg", |a: f64| -a).unwrap())
                .unwrap();
            globals
                .set(
                    "add",
                    Function::new2(ctx, "add", |a: f64, b: f64| a + b).unwrap(),
                )
                .unwrap();

            let r: f64 = ctx.eval("neg(add(one(), 2))").unwrap();
            assert_eq!(r, -3.0);
        })
    }

    #[test]
    fn mut_callback2() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let globals = ctx.globals();
            let mut id_alloc = 0;
            globals
                .set(
                    "new_id",
                    Function::new_mut2(ctx, "id", move || {
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
    fn callback2_with_ctx_and_this() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let globals = ctx.globals();
            let mut id_alloc = 0;
            globals
                .set(
                    "new_id",
                    move |ctx: Ctx<'_>, _this: This<()>| -> Result<_> {
                        let initial: Option<u32> = ctx.globals().get("initial_id")?;
                        if let Some(initial) = initial {
                            id_alloc += 1;
                            Ok(id_alloc + initial)
                        } else {
                            Err(Error::Unknown)
                        }
                    }
                    .into_function(ctx, "id")
                    .unwrap(),
                )
                .unwrap();

            //let _err = ctx.eval::<u32, _>("new_id()").unwrap_err();
            globals.set("initial_id", 10).unwrap();

            let id: u32 = ctx.eval("new_id()").unwrap();
            assert_eq!(id, 11);
            let id: u32 = ctx.eval("new_id()").unwrap();
            assert_eq!(id, 12);
            let id: u32 = ctx.eval("new_id()").unwrap();
            assert_eq!(id, 13);
        })
    }
}
