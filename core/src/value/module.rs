//! Types for loading and handling JS modules.

use std::{
    ffi::{CStr, CString},
    mem::MaybeUninit,
    ptr::{self, NonNull},
    slice,
};

use crate::{qjs, Context, Ctx, Error, IntoJs, Promise, Result, Value};

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
        let _ = exports;
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

    pub fn declare_def<D, N>(ctx: Ctx<'js>, name: N) -> Result<Module<'js>>
    where
        N: Into<Vec<u8>>,
        D: ModuleDef,
    {
        todo!()
        //Ok(())
    }

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
}
