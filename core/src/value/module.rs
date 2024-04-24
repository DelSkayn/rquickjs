//! Types for loading and handling JS modules.

use std::{
    ffi::{CStr, CString},
    mem::MaybeUninit,
    ptr, slice,
};

use crate::{
    atom::PredefinedAtom, qjs, Atom, Context, Ctx, Error, FromAtom, FromJs, IntoAtom, IntoJs,
    Object, Promise, Result, Value,
};

/// Helper macro to provide module init function.
/// Use for exporting module definitions to be loaded as part of a dynamic library.
/// ```
/// use rquickjs::{module::ModuleDef, module_init};
///
/// struct MyModule;
/// impl ModuleDef for MyModule {}
///
/// module_init!(MyModule);
/// // or
/// module_init!(js_init_my_module: MyModule);
/// ```
#[macro_export]
macro_rules! module_init {
    ($type:ty) => {
        $crate::module_init!(js_init_module: $type);
    };

    ($name:ident: $type:ty) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(
            ctx: *mut $crate::qjs::JSContext,
            module_name: *const $crate::qjs::c_char,
        ) -> *mut $crate::qjs::JSModuleDef {
            $crate::Module::init_raw::<$type>(ctx, module_name)
        }
    };
}

/// The raw module load function (`js_module_init`)
pub type ModuleLoadFn =
    unsafe extern "C" fn(*mut qjs::JSContext, *const qjs::c_char) -> *mut qjs::JSModuleDef;

pub trait ModuleDef {
    fn declare<'js>(decl: &Declarations<'js>) -> Result<()> {
        let _ = decl;
        Ok(())
    }

    fn evaluate<'js>(ctx: &Ctx<'js>, exports: &Exports<'js>) -> Result<()> {
        let _ = (exports, ctx);
        Ok(())
    }
}

pub struct Declarations<'js>(Module<'js>);

impl<'js> Declarations<'js> {
    /// Define a new export in a module.
    pub fn declare<N>(&self, name: N) -> Result<&Self>
    where
        N: Into<Vec<u8>>,
    {
        let name = CString::new(name)?;
        self.declare_c_str(name.as_c_str())
    }

    /// Define a new export in a module.
    ///
    /// This function avoids an extra allocation, having to convert from a rust string into a
    /// null-terminated CStr.
    pub fn declare_c_str(&self, name: &CStr) -> Result<&Self> {
        unsafe { qjs::JS_AddModuleExport(self.0.ctx().as_ptr(), self.0.as_ptr(), name.as_ptr()) };
        Ok(self)
    }
}

pub struct Exports<'js>(Module<'js>);

impl<'js> Exports<'js> {
    /// Set the value of an exported entry.
    pub fn export<N: Into<Vec<u8>>, T: IntoJs<'js>>(&self, name: N, value: T) -> Result<&Self> {
        let name = CString::new(name.into())?;
        self.export_c_str(name.as_c_str(), value)
    }

    /// Set the value of an exported entry.
    ///
    /// This function avoids a possible conversion from a rust string into a CStr
    pub fn export_c_str<T: IntoJs<'js>>(&self, name: &CStr, value: T) -> Result<&Self> {
        let value = value.into_js(self.0.ctx())?;
        let res = unsafe {
            qjs::JS_SetModuleExport(
                self.0.ctx().as_ptr(),
                self.0.as_ptr(),
                name.as_ptr(),
                value.into_js_value(),
            )
        };
        if res < 0 {
            return Err(Error::InvalidExport);
        }

        Ok(self)
    }
}

#[derive(Clone, Debug)]
pub struct Module<'js>(pub(crate) Value<'js>);

impl<'js> Module<'js> {
    pub(crate) fn as_ptr(&self) -> *mut qjs::JSModuleDef {
        unsafe { qjs::JS_VALUE_GET_PTR(self.as_js_value()).cast() }
    }

    /// Declare a module but don't evaluate it.
    pub fn declare<N, S>(ctx: Ctx<'js>, name: N, source: S) -> Result<Module<'js>>
    where
        N: Into<Vec<u8>>,
        S: Into<Vec<u8>>,
    {
        let name = CString::new(name)?;
        let flag =
            qjs::JS_EVAL_TYPE_MODULE | qjs::JS_EVAL_FLAG_STRICT | qjs::JS_EVAL_FLAG_COMPILE_ONLY;

        let module_val = unsafe { ctx.eval_raw(source, name.as_c_str(), flag as i32)? };
        let module_val = unsafe { ctx.handle_exception(module_val)? };
        debug_assert_eq!(qjs::JS_TAG_MODULE, unsafe {
            qjs::JS_VALUE_GET_TAG(module_val)
        });
        unsafe { Ok(Module::from_js_value(ctx, module_val)) }
    }

    /// Declare a rust native module but don't evaluate it.
    pub fn declare_def<D, N>(ctx: Ctx<'js>, name: N) -> Result<Module<'js>>
    where
        N: Into<Vec<u8>>,
        D: ModuleDef,
    {
        let name = CString::new(name)?;
        let ptr =
            unsafe { qjs::JS_NewCModule(ctx.as_ptr(), name.as_ptr(), Some(Self::eval_fn::<D>)) };
        let value = qjs::JS_MKPTR(qjs::JS_TAG_MODULE, ptr.cast());
        let m = unsafe { Module::from_js_value_const(ctx, value) };

        let decl = Declarations(m);
        D::declare(&decl)?;

        Ok(decl.0)
        //Ok(())
    }

    unsafe extern "C" fn eval_fn<D>(
        ctx: *mut qjs::JSContext,
        ptr: *mut qjs::JSModuleDef,
    ) -> qjs::c_int
    where
        D: ModuleDef,
    {
        let ctx = Ctx::from_ptr(ctx);
        // Should never be null
        debug_assert_ne!(ptr, ptr::null_mut());
        let value = qjs::JS_MKPTR(qjs::JS_TAG_MODULE, ptr.cast());
        let module = unsafe { Module::from_js_value_const(ctx.clone(), value) };
        let exports = Exports(module);
        match D::evaluate(&ctx, &exports) {
            Ok(_) => 0,
            Err(error) => {
                error.throw(&ctx);
                -1
            }
        }
    }

    /// Evaluate the source of a module.
    ///
    /// This function returns a promise which resolved when the modules was fully compiled and
    /// returns undefined.
    ///
    /// If the module itself is required, you should first declare it and then call eval on the
    /// module.
    pub fn evaluate<N, S>(ctx: Ctx<'js>, name: N, source: S) -> Result<Promise<'js>>
    where
        N: Into<Vec<u8>>,
        S: Into<Vec<u8>>,
    {
        let name = CString::new(name)?;
        let flag = qjs::JS_EVAL_TYPE_MODULE | qjs::JS_EVAL_FLAG_STRICT;

        let module_val = unsafe { ctx.eval_raw(source, name.as_c_str(), flag as i32)? };
        let module_val = unsafe { ctx.handle_exception(module_val)? };
        let v = unsafe { Value::from_js_value(ctx, module_val) };
        Ok(v.into_promise().expect("evaluate should return a promise"))
    }

    /// Declares a module in the runtime and evaluates it.
    pub fn evaluate_def<D, N>(ctx: Ctx<'js>, name: N) -> Result<Promise<'js>>
    where
        N: Into<Vec<u8>>,
        D: ModuleDef,
    {
        let module = Self::declare_def::<D, N>(ctx, name)?;
        module.eval()
    }

    /// Load a module from quickjs bytecode.
    ///
    /// # Safety
    /// User must ensure that bytes handed to this function contain valid bytecode.
    pub unsafe fn load(ctx: Ctx<'js>, bytes: &[u8]) -> Result<Module<'js>> {
        let module = unsafe {
            qjs::JS_ReadObject(
                ctx.as_ptr(),
                bytes.as_ptr(),
                bytes.len() as _,
                (qjs::JS_READ_OBJ_BYTECODE | qjs::JS_READ_OBJ_ROM_DATA) as i32,
            )
        };
        let module = ctx.handle_exception(module)?;
        unsafe { Ok(Module::from_js_value(ctx, module)) }
    }

    /// Load a module from a raw module loading function.
    ///
    /// # Safety
    /// The soundness of this function depends completely on if load_fn is implemented correctly.
    /// THe load_fn function must return a pointer to a valid module loaded with the given context.
    pub unsafe fn from_load_fn<N>(
        ctx: Ctx<'js>,
        name: N,
        load_fn: ModuleLoadFn,
    ) -> Result<Module<'js>>
    where
        N: Into<Vec<u8>>,
    {
        let name = CString::new(name)?;
        let ptr = (load_fn)(ctx.as_ptr(), name.as_ptr().cast());
        let val = qjs::JS_MKPTR(qjs::JS_TAG_MODULE, ptr.cast());
        unsafe { Ok(Module::from_js_value(ctx, val)) }
    }

    /// Evaluate the module.
    ///
    /// Returns a promise which resolves when the module has completely resolved.
    /// The return value of the promise is the JavaScript value undefined.
    pub fn eval(&self) -> Result<Promise<'js>> {
        // JS_EvalFunction `free's` the module so we should dup first
        let ret = unsafe {
            qjs::JS_EvalFunction(self.ctx().as_ptr(), qjs::JS_DupValue(self.as_js_value()))
        };
        let ret = unsafe { self.ctx.handle_exception(ret)? };
        let v = unsafe { Value::from_js_value(self.ctx().clone(), ret) };
        Ok(v.into_promise().expect("evaluate should return a promise"))
    }

    /// Write object bytecode for the module in little endian format.
    pub fn write_le(&self) -> Result<Vec<u8>> {
        let swap = cfg!(target_endian = "big");
        self.write(swap)
    }

    /// Write object bytecode for the module in big endian format.
    pub fn write_be(&self) -> Result<Vec<u8>> {
        let swap = cfg!(target_endian = "little");
        self.write(swap)
    }

    /// Write object bytecode for the module.
    ///
    /// `swap_endianess` swaps the endianness of the bytecode, if true, from native to the other
    /// kind. Use if the bytecode is meant for a target with a different endianness than the
    /// current.
    pub fn write(&self, swap_endianess: bool) -> Result<Vec<u8>> {
        let ctx = &self.ctx;
        let mut len = MaybeUninit::uninit();
        // TODO: Allow inclusion of other flags?
        let mut flags = qjs::JS_WRITE_OBJ_BYTECODE;
        if swap_endianess {
            flags |= qjs::JS_WRITE_OBJ_BSWAP;
        }
        let buf = unsafe {
            qjs::JS_WriteObject(
                ctx.as_ptr(),
                len.as_mut_ptr(),
                self.as_js_value(),
                flags as i32,
            )
        };
        if buf.is_null() {
            return Err(ctx.raise_exception());
        }
        let len = unsafe { len.assume_init() };
        let obj = unsafe { slice::from_raw_parts(buf, len as _) };
        let obj = Vec::from(obj);
        unsafe { qjs::js_free(ctx.as_ptr(), buf as _) };
        Ok(obj)
    }

    /// A function for loading a Rust module from C.
    ///
    /// # Safety
    /// This function should only be called when the module is loaded as part of a dynamically
    /// loaded library.
    pub unsafe extern "C" fn init_raw<D>(
        ctx: *mut qjs::JSContext,
        name: *const qjs::c_char,
    ) -> *mut qjs::JSModuleDef
    where
        D: ModuleDef,
    {
        Context::init_raw(ctx);
        let ctx = Ctx::from_ptr(ctx);
        let name = CStr::from_ptr(name).to_bytes();
        match Self::declare_def::<D, _>(ctx.clone(), name) {
            Ok(module) => module.as_ptr(),
            Err(error) => {
                error.throw(&ctx);
                ptr::null_mut()
            }
        }
    }

    /// Returns the name of the module
    pub fn name<N>(&self) -> Result<N>
    where
        N: FromAtom<'js>,
    {
        let name = unsafe {
            Atom::from_atom_val(
                self.ctx.clone(),
                qjs::JS_GetModuleName(self.ctx.as_ptr(), self.as_ptr()),
            )
        };
        N::from_atom(name)
    }

    /// Return the `import.meta` object of a module
    pub fn meta<T>(&self) -> Result<T>
    where
        T: FromJs<'js>,
    {
        let meta = unsafe {
            Value::from_js_value(
                self.ctx.clone(),
                self.ctx
                    .handle_exception(qjs::JS_GetImportMeta(self.ctx.as_ptr(), self.as_ptr()))?,
            )
        };
        T::from_js(&self.ctx, meta)
    }

    /// Import and evaluate a module
    ///
    /// This will work similar to an `import(specifier)` statement in JavaScript returning a promise with the result of the imported module.
    pub fn import<S: Into<Vec<u8>>>(ctx: &Ctx<'js>, specifier: S) -> Result<Promise<'js>> {
        let specifier = CString::new(specifier)?;
        unsafe {
            let base_name = ctx
                .script_or_module_name(1)
                .unwrap_or_else(|| Atom::from_predefined(ctx.clone(), PredefinedAtom::Empty));

            let base_name_c_str = qjs::JS_AtomToCString(ctx.as_ptr(), base_name.atom);

            let res = qjs::JS_LoadModule(ctx.as_ptr(), base_name_c_str, specifier.as_ptr());

            qjs::JS_FreeCString(ctx.as_ptr(), base_name_c_str);

            let res = ctx.handle_exception(res)?;

            Ok(Promise::from_js_value(ctx.clone(), res))
        }
    }

    /// Returns the module namespace, an object containing all the module exported values.
    pub fn namespace(&self) -> Result<Object<'js>> {
        unsafe {
            let v = qjs::JS_GetModuleNamespace(self.ctx().as_ptr(), self.as_ptr());
            let v = self.ctx().handle_exception(v)?;
            Ok(Object::from_js_value(self.ctx().clone(), v))
        }
    }

    /// Return exported value by name
    pub fn get<N, T>(&self, name: N) -> Result<T>
    where
        N: IntoAtom<'js>,
        T: FromJs<'js>,
    {
        self.namespace()?.get(name)
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::*;

    pub struct RustModule;

    impl ModuleDef for RustModule {
        fn declare(define: &Declarations) -> Result<()> {
            define.declare_c_str(CStr::from_bytes_with_nul(b"hello\0")?)?;
            Ok(())
        }

        fn evaluate<'js>(_ctx: &Ctx<'js>, exports: &Exports<'js>) -> Result<()> {
            exports.export_c_str(CStr::from_bytes_with_nul(b"hello\0")?, "world")?;
            Ok(())
        }
    }

    pub struct CrashingRustModule;

    impl ModuleDef for CrashingRustModule {
        fn declare(_: &Declarations) -> Result<()> {
            Ok(())
        }

        fn evaluate<'js>(ctx: &Ctx<'js>, _exports: &Exports<'js>) -> Result<()> {
            ctx.eval::<(), _>(r#"throw new Error("kaboom")"#)?;
            Ok(())
        }
    }

    #[test]
    fn from_rust_def() {
        test_with(|ctx| {
            Module::declare_def::<RustModule, _>(ctx, "rust_mod").unwrap();
        })
    }

    #[test]
    fn from_rust_def_eval() {
        test_with(|ctx| {
            let _ = Module::evaluate_def::<RustModule, _>(ctx, "rust_mod").unwrap();
        })
    }

    #[test]
    fn import_native() {
        test_with(|ctx| {
            Module::declare_def::<RustModule, _>(ctx.clone(), "rust_mod").unwrap();
            Module::evaluate(
                ctx.clone(),
                "test",
                r#"
                import { hello } from "rust_mod";

                globalThis.hello = hello;
            "#,
            )
            .unwrap()
            .finish::<()>()
            .unwrap();
            let text = ctx
                .globals()
                .get::<_, String>("hello")
                .unwrap()
                .to_string()
                .unwrap();
            assert_eq!(text.as_str(), "world");
        })
    }

    #[test]
    fn import_async() {
        test_with(|ctx| {
            Module::declare(
                ctx.clone(),
                "rust_mod",
                "
                async function foo(){
                    return 'world';
                };
                export let hello = await foo();
            ",
            )
            .unwrap();
            Module::evaluate(
                ctx.clone(),
                "test",
                r#"
                import { hello } from "rust_mod";
                globalThis.hello = hello;
            "#,
            )
            .unwrap()
            .finish::<()>()
            .unwrap();
            let text = ctx
                .globals()
                .get::<_, String>("hello")
                .unwrap()
                .to_string()
                .unwrap();
            assert_eq!(text.as_str(), "world");
        })
    }

    #[test]
    fn import() {
        test_with(|ctx| {
            Module::declare_def::<RustModule, _>(ctx.clone(), "rust_mod").unwrap();
            let val: Object = Module::import(&ctx, "rust_mod").unwrap().finish().unwrap();
            let hello: StdString = val.get("hello").unwrap();

            assert_eq!(&hello, "world");
        })
    }

    #[test]
    #[should_panic(expected = "kaboom")]
    fn import_crashing() {
        use crate::{CatchResultExt, Context, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            Module::declare_def::<CrashingRustModule, _>(ctx.clone(), "bad_rust_mod").unwrap();
            let _: Value = Module::import(&ctx, "bad_rust_mod")
                .catch(&ctx)
                .unwrap()
                .finish()
                .catch(&ctx)
                .unwrap();
        });
    }

    #[test]
    fn eval_crashing_module_inside_module() {
        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();

        ctx.with(|ctx| {
            let globals = ctx.globals();
            let eval_crashing = |ctx: Ctx| {
                Module::evaluate(ctx, "test2", "throw new Error(1)").map(|x| x.finish::<()>())
            };
            let function = Function::new(ctx.clone(), eval_crashing).unwrap();
            globals.set("eval_crashing", function).unwrap();

            let res = Module::evaluate(ctx, "test", " eval_crashing(); ")
                .unwrap()
                .finish::<()>();
            assert!(res.is_err())
        });
    }

    #[test]
    fn from_javascript() {
        test_with(|ctx| {
            let module = Module::declare(
                ctx.clone(),
                "Test",
                r#"
            export var a = 2;
            export function foo(){ return "bar"}
            export class Baz{
                quel = 3;
                constructor(){
                }
            }
                "#,
            )
            .unwrap();

            module.eval().unwrap().finish::<()>().unwrap();

            assert_eq!(module.name::<StdString>().unwrap(), "Test");
            let _ = module.meta::<Object>().unwrap();

            let names = module
                .namespace()
                .unwrap()
                .keys()
                .collect::<Result<Vec<StdString>>>()
                .unwrap();

            assert_eq!(names[0], "a");
            assert_eq!(names[1], "foo");
            assert_eq!(names[2], "Baz");

            let entries = module
                .namespace()
                .unwrap()
                .props()
                .collect::<Result<Vec<(StdString, Value)>>>()
                .unwrap();

            assert_eq!(entries[0].0, "a");
            assert_eq!(i32::from_js(&ctx, entries[0].1.clone()).unwrap(), 2);
            assert_eq!(entries[1].0, "foo");
            assert_eq!(entries[2].0, "Baz");
        });
    }
}
