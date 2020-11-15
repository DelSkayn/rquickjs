use crate::{
    value::rf::JsObjectRef, Ctx, FromJs, FromJsMulti, Object, Result, ToJs, ToJsMulti, Value,
};
use rquickjs_sys as qjs;
use std::{ffi::CString, mem, os::raw::c_int, ptr};

mod ffi;
use ffi::FuncOpaque;

/// A trait which allows rquickjs to create a callback with only minimal overhead.
pub trait StaticFn<'js> {
    /// type of the this object that the function expects
    /// Generally the global object.
    type This: FromJs<'js>;
    /// type of the arguments that the function expects
    type Args: FromJsMulti<'js>;
    /// type of the return value
    type Result: ToJs<'js>;

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
        N: Into<Vec<u8>>,
        F: StaticFn<'js>,
    {
        let name = CString::new(name)?;
        let func = ffi::call_fn_static::<F>
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
                F::Args::LEN as c_int,
                qjs::JSCFunctionEnum_JS_CFUNC_generic,
                0,
            );
            Ok(Function(JsObjectRef::from_js_value(ctx, val)))
        }
    }

    #[cfg(not(feature = "parallel"))]
    pub fn new_mut<F, A, T, R, N>(ctx: Ctx<'js>, name: N, func: F) -> Result<Self>
    where
        N: Into<Vec<u8>>,
        A: FromJsMulti<'js>,
        T: FromJs<'js>,
        R: ToJs<'js>,
        F: FnMut(Ctx<'js>, T, A) -> Result<R> + 'static,
    {
        unsafe {
            let opaque = ffi::wrap_cb_mut(func);
            Self::new_unsafe(ctx, CString::new(name)?, opaque)
        }
    }

    #[cfg(not(feature = "parallel"))]
    pub fn new<F, A, T, R, N>(ctx: Ctx<'js>, name: N, func: F) -> Result<Self>
    where
        N: Into<Vec<u8>>,
        A: FromJsMulti<'js>,
        T: FromJs<'js>,
        R: ToJs<'js>,
        F: Fn(Ctx<'js>, T, A) -> Result<R> + 'static,
    {
        unsafe {
            let opaque = ffi::wrap_cb(func);
            Self::new_unsafe(ctx, CString::new(name)?, opaque)
        }
    }

    #[cfg(feature = "parallel")]
    pub fn new_mut<F, A, T, R, N>(ctx: Ctx<'js>, name: N, func: F) -> Result<Self>
    where
        N: Into<Vec<u8>>,
        A: FromJsMulti<'js>,
        T: FromJs<'js>,
        R: ToJs<'js>,
        F: FnMut(Ctx<'js>, T, A) -> Result<R> + Send + 'static,
    {
        unsafe {
            let opaque = ffi::wrap_cb_mut(func);
            Self::new_unsafe(ctx, CString::new(name)?, opaque)
        }
    }

    #[cfg(feature = "parallel")]
    pub fn new<F, A, T, R, N>(ctx: Ctx<'js>, name: N, func: F) -> Result<Self>
    where
        N: Into<Vec<u8>>,
        A: FromJsMulti<'js>,
        T: FromJs<'js>,
        R: ToJs<'js>,
        F: Fn(Ctx<'js>, T, A) -> Result<R> + Send + 'static,
    {
        unsafe {
            let opaque = ffi::wrap_cb(func);
            Self::new_unsafe(ctx, CString::new(name)?, opaque)
        }
    }

    unsafe fn new_unsafe(ctx: Ctx<'js>, _name: CString, opaque: FuncOpaque) -> Result<Self> {
        let class_id = ctx.get_opaque().func_class;
        let rt = qjs::JS_GetRuntime(ctx.ctx);
        if qjs::JS_IsRegisteredClass(rt, class_id) == 0 {
            let class_def = qjs::JSClassDef {
                class_name: b"RustFunc\0".as_ptr() as *const _,
                finalizer: Some(ffi::cb_finalizer),
                gc_mark: None,
                call: Some(ffi::cb_call),
                exotic: ptr::null_mut(),
            };
            assert!(qjs::JS_NewClass(rt, class_id, &class_def) == 0);
        }
        let obj = qjs::JS_NewObjectClass(ctx.ctx, class_id as i32);
        qjs::JS_SetOpaque(obj, Box::into_raw(Box::new(opaque)) as *mut _);
        Ok(Function(JsObjectRef::from_js_value(ctx, obj)))
    }

    /// Call a function with given arguments with the `this` as the global context object.
    pub fn call<A, R>(&self, args: A) -> Result<R>
    where
        A: ToJsMulti<'js>,
        R: FromJs<'js>,
    {
        self.call_on(self.0.ctx.globals(), args)
    }

    /// Call a function with given arguments on a given `this`
    pub fn call_on<A, T, R>(&self, this: T, args: A) -> Result<R>
    where
        A: ToJsMulti<'js>,
        R: FromJs<'js>,
        T: ToJs<'js>,
    {
        let args = args.to_js_multi(self.0.ctx)?;
        let this = this.to_js(self.0.ctx)?;
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
            let f = Function::new_static::<Test, _>(ctx, "test").unwrap();
            let eval: Function = ctx.eval("(a) => { a()}").unwrap();
            eval.call::<_, ()>(f.clone()).unwrap();
            f.call::<_, ()>(()).unwrap()
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
            let eval: Function = ctx.eval("(a) => { a()}").unwrap();
            eval.call::<_, ()>(f.clone()).unwrap();
            f.call::<_, ()>(()).unwrap();
            assert!(*called.lock().unwrap())
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
                dbg!(v);
                return Ok(v);
            })
            .unwrap();
            let eval: Function = ctx.eval("(a) => { return a()}").unwrap();
            assert_eq!(eval.call::<_, i32>(f.clone()).unwrap(), 1);
            assert_eq!(eval.call::<_, i32>(f.clone()).unwrap(), 2);
            assert_eq!(eval.call::<_, i32>(f.clone()).unwrap(), 3);
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
}
