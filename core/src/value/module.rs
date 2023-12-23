//! Types for loading and handling JS modules.

use std::{
    borrow::Cow,
    collections::HashSet,
    ffi::{CStr, CString},
    fmt,
    mem::MaybeUninit,
    ptr::{self, NonNull},
    slice,
};

#[cfg(feature = "exports")]
use std::marker::PhantomData;

use crate::{qjs, Atom, Context, Ctx, Error, FromAtom, FromJs, IntoJs, Result, Value};

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

/// An enum containing all possible ways to declare an module.
#[derive(Clone)]
pub enum ModuleDataKind {
    /// Module source text,
    Source(Vec<u8>),
    /// A function which loads a module from Rust.
    Native(for<'js> unsafe fn(ctx: &Ctx<'js>, name: Vec<u8>) -> Result<Module<'js>>),
    /// A raw loading function, used for loading from dynamic libraries.
    Raw(ModuleLoadFn),
    /// Module object bytecode.
    ByteCode(Cow<'static, [u8]>),
}

// Debug could not be derived on stable because the fn only implemented it for a specific lifetime
// <'js> not a general one.
//
// Remove when stable has caught up to nightly..
impl fmt::Debug for ModuleDataKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ModuleDataKind::Source(ref x) => {
                f.debug_tuple("ModuleDataKind::Source").field(x).finish()
            }
            ModuleDataKind::Raw(ref x) => f.debug_tuple("ModuleDataKind::Raw").field(x).finish(),
            ModuleDataKind::ByteCode(ref x) => {
                f.debug_tuple("ModuleDataKind::ByteCode").field(x).finish()
            }
            ModuleDataKind::Native(_) => f
                .debug_tuple("ModuleDataKind::ByteCode")
                .field(&"<native function>")
                .finish(),
        }
    }
}

impl ModuleDataKind {
    unsafe fn declare<'js, N: Into<Vec<u8>>>(self, ctx: Ctx<'js>, name: N) -> Result<Module<'js>> {
        match self {
            ModuleDataKind::Source(x) => Module::unsafe_declare(ctx, name, x),
            ModuleDataKind::Native(x) => (x)(&ctx, name.into()),
            ModuleDataKind::Raw(x) => {
                let name = CString::new(name)?;
                let ptr = (x)(ctx.as_ptr(), name.as_ptr().cast());
                let ptr = NonNull::new(ptr).ok_or(Error::Unknown)?;
                Ok(Module::from_module_def(ctx, ptr))
            }
            ModuleDataKind::ByteCode(x) => Module::unsafe_declare_read_object(ctx, x.as_ref()),
        }
    }
}

/// The data required to load a module, either from source or native.
#[derive(Clone, Debug)]
pub struct ModuleData {
    name: Vec<u8>,
    data: ModuleDataKind,
}

impl ModuleData {
    /// Create module data for a module loaded from source.
    pub fn source<N, S>(name: N, source: S) -> Self
    where
        N: Into<Vec<u8>>,
        S: Into<Vec<u8>>,
    {
        ModuleData {
            name: name.into(),
            data: ModuleDataKind::Source(source.into()),
        }
    }

    /// Create module data for a module loaded from source.
    ///
    /// # Safety
    /// User must ensure that the bytecode is valid QuickJS bytecode.
    pub unsafe fn bytecode<N, S>(name: N, bytecode: S) -> Self
    where
        N: Into<Vec<u8>>,
        S: Into<Cow<'static, [u8]>>,
    {
        ModuleData {
            name: name.into(),
            data: ModuleDataKind::ByteCode(bytecode.into()),
        }
    }

    /// Create module data for a module loaded from a native Rust definition.
    pub fn native<D, N>(name: N) -> Self
    where
        D: ModuleDef,
        N: Into<Vec<u8>>,
    {
        unsafe fn define<'js, D: ModuleDef>(ctx: &Ctx<'js>, name: Vec<u8>) -> Result<Module<'js>> {
            Module::unsafe_declare_def::<D, _>(ctx.clone(), name)
        }

        ModuleData {
            name: name.into(),
            data: ModuleDataKind::Native(define::<D>),
        }
    }

    /// Create module data for a module loaded from a native Rust definition.
    ///
    /// # Safety
    /// User must ensure that `load_fn` behaves like a loader function.
    ///
    /// The function must take a context and a c string without taking ownership of either values
    /// and return a pointer to `qjs::JSModuleDef`, or a null pointer if there was some error.
    pub unsafe fn raw<N>(name: N, load_fn: ModuleLoadFn) -> Self
    where
        N: Into<Vec<u8>>,
    {
        ModuleData {
            name: name.into(),
            data: ModuleDataKind::Raw(load_fn),
        }
    }

    /// Returns the kind of `ModuleData`.
    pub fn kind(&self) -> &ModuleDataKind {
        &self.data
    }

    /// Declare the module defined in the `ModuleData`.
    pub fn declare<'js>(self, ctx: Ctx<'js>) -> Result<()> {
        unsafe {
            let _ = self.unsafe_declare(ctx)?;
        }
        Ok(())
    }

    /// Declare the module defined in the `ModuleData`.
    ///
    /// # Safety
    /// This method returns an unevaluated module.
    /// It is UB to hold unto unevaluated modules across any call to  a module function which can
    /// invalidate unevaluated modules and returned an error.
    pub unsafe fn unsafe_declare<'js>(self, ctx: Ctx<'js>) -> Result<Module<'js>> {
        self.data.declare(ctx, self.name)
    }
}

/// A struct for loading multiple modules at once safely.
///
/// Modules are built in two steps, declare and evaluate. During evaluation a module might import
/// another module, if no such declared module exist the evaluation fails.
///
/// This struct allows one to first declare all possible modules and then evaluate them allowing
/// modules to import each other.
///
/// Only use if you need to acquire the module objects after. Otherwise, it is easier to use the
/// other safe methods on the [`Module`] struct like [`Module::evaluate`]
pub struct ModulesBuilder {
    modules: Vec<ModuleData>,
}

impl ModulesBuilder {
    pub fn new() -> Self {
        ModulesBuilder {
            modules: Vec::new(),
        }
    }

    pub fn source<N, S>(mut self, name: N, source: S) -> Self
    where
        N: Into<Vec<u8>>,
        S: Into<Vec<u8>>,
    {
        self.with_source(name, source);
        self
    }

    pub fn with_source<N, S>(&mut self, name: N, source: S) -> &mut Self
    where
        N: Into<Vec<u8>>,
        S: Into<Vec<u8>>,
    {
        self.modules.push(ModuleData::source(name, source));
        self
    }

    pub fn native<D, N>(mut self, name: N) -> Self
    where
        D: ModuleDef,
        N: Into<Vec<u8>>,
    {
        self.with_native::<D, _>(name);
        self
    }

    pub fn with_native<D, N>(&mut self, name: N) -> &mut Self
    where
        D: ModuleDef,
        N: Into<Vec<u8>>,
    {
        self.modules.push(ModuleData::native::<D, N>(name));
        self
    }

    pub fn eval<'js>(self, ctx: &Ctx<'js>) -> Result<Vec<Module<'js>>> {
        let mut modules = Vec::with_capacity(self.modules.len());
        for m in self
            .modules
            .into_iter()
            .map(|x| unsafe { x.unsafe_declare(ctx.clone()) })
        {
            modules.push(m?);
        }

        for m in modules.iter() {
            // Safety: This is save usage of the modules since if any fail to evaluate we
            // immediately return and drop all modules
            unsafe { m.eval()? }
        }
        // All modules evaluated without error so they are all still alive.
        Ok(modules)
    }
}

impl Default for ModulesBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Module declarations.
///
/// Struct used in the [`ModuleDef`] trait for declaring module exports.
pub struct Declarations {
    declarations: HashSet<Cow<'static, CStr>>,
}

impl Declarations {
    pub(crate) fn new() -> Self {
        Declarations {
            declarations: HashSet::new(),
        }
    }

    /// Define a new export in a module.
    pub fn declare<N>(&mut self, name: N) -> Result<&mut Self>
    where
        N: Into<Vec<u8>>,
    {
        self.declarations.insert(Cow::Owned(CString::new(name)?));
        Ok(self)
    }

    /// Define a new export in a module using a static `CStr`.
    ///
    /// This method is can be used to avoid some allocation in the case that the name is static.
    pub fn declare_static(&mut self, name: &'static CStr) -> Result<&mut Self> {
        self.declarations.insert(Cow::Borrowed(name));
        Ok(self)
    }

    pub(crate) unsafe fn apply(self, ctx: Ctx<'_>, module: &Module) -> Result<()> {
        for k in self.declarations {
            let ptr = match k {
                Cow::Borrowed(x) => x.as_ptr(),
                Cow::Owned(x) => x.into_raw(),
            };
            let res = unsafe {
                qjs::JS_AddModuleExport(ctx.as_ptr(), module.as_module_def().as_ptr(), ptr)
            };
            if res < 0 {
                return Err(Error::Allocation);
            }
        }
        Ok(())
    }

    /// Returns an iterator over existing declarations.
    pub fn iter(&self) -> impl Iterator<Item = &Cow<'static, CStr>> {
        self.declarations.iter()
    }
}

struct Export<'js> {
    name: CString,
    value: Value<'js>,
}

/// A struct used to load the exports of a module.
///
/// Used in the `ModuleDef::load`.
pub struct Exports<'js> {
    ctx: Ctx<'js>,
    exports: Vec<Export<'js>>,
}

impl<'js> Exports<'js> {
    pub(crate) fn new(ctx: Ctx<'js>) -> Self {
        Exports {
            ctx,
            exports: Vec::new(),
        }
    }

    /// Export a new value from the module.
    pub fn export<N: Into<Vec<u8>>, T: IntoJs<'js>>(
        &mut self,
        name: N,
        value: T,
    ) -> Result<&mut Self> {
        let name = CString::new(name.into())?;
        let value = value.into_js(&self.ctx)?;
        self.export_value(name, value)
    }

    /// Export a new value from the module.
    pub fn export_value(&mut self, name: CString, value: Value<'js>) -> Result<&mut Self> {
        self.exports.push(Export { name, value });
        Ok(self)
    }

    pub(crate) unsafe fn apply(self, module: Module) -> Result<()> {
        for export in self.exports {
            let name = export.name;
            let value = export.value;

            let res = unsafe {
                // Ownership of name is retained
                // Ownership of value is transferred.
                qjs::JS_SetModuleExport(
                    self.ctx.as_ptr(),
                    module.as_module_def().as_ptr(),
                    name.as_ref().as_ptr(),
                    value.into_js_value(),
                )
            };

            // The below is wrong, if a value is applied but not previously defined the above
            // function can error!
            //
            // WRONG: previous checks and the fact that we also previously added the export should ensure
            // that the only error can be an allocation error.
            if res < 0 {
                return Err(Error::Unknown);
            }
        }
        Ok(())
    }

    /// Returns an iterator over existing imports.
    pub fn iter(&self) -> impl Iterator<Item = (&CStr, &Value<'js>)> {
        self.exports.iter().map(|x| (x.name.as_c_str(), &x.value))
    }
}

/// A JavaScript module.
///
/// # Safety
/// In QuickJS JavaScript modules are loaded in two steps. First a module is declared, the runtime
/// is made aware of its existence, given a name, and its exports are defined.
///
/// Then after declaration the module is evaluated, the module's exports are actually given a
/// value.
///
/// QuickJS handles module lifetime differently then other JavaScript values. Modules live, once
/// evaluated, for the entire lifetime of the runtime. However before they are evaluated they can
/// be removed at any point.
///
/// Specifically all unevaluated modules are removed if any module generates an error during either
/// declaration or evaluation. As a result while it is possible to acquire a unevaluated module, it
/// is unsafe to hold onto such a module and any function which returns such an unevaluated modules
/// is marked as unsafe.
#[derive(Clone)]
#[must_use = "if access to the module object is not required, prefer to only declare a module"]
pub struct Module<'js> {
    ctx: Ctx<'js>,
    /// Module lifetime, behave differently then normal lifetimes.
    /// A module lives for the entire lifetime of the runtime, so we don't need to keep track of
    /// reference counts.
    module: NonNull<qjs::JSModuleDef>,
}

/// Module definition trait
pub trait ModuleDef {
    fn declare(declare: &mut Declarations) -> Result<()> {
        let _ = declare;
        Ok(())
    }

    /// The exports should be added here
    fn evaluate<'js>(_ctx: &Ctx<'js>, exports: &mut Exports<'js>) -> Result<()> {
        let _ = exports;
        Ok(())
    }
}

impl<'js> Module<'js> {
    pub(crate) fn from_module_def(ctx: Ctx<'js>, def: NonNull<qjs::JSModuleDef>) -> Self {
        Module { ctx, module: def }
    }

    pub(crate) fn as_module_def(&self) -> NonNull<qjs::JSModuleDef> {
        self.module
    }

    /// Declares a new JS module in the context.
    ///
    /// This function doesn't return a module since holding on to unevaluated modules is unsafe.
    /// If you need to hold onto unsafe modules use the [`Module::unsafe_declare`] functions.
    ///
    /// It is unsafe to hold onto unevaluated modules across this call.
    pub fn declare<N, S>(ctx: Ctx<'js>, name: N, source: S) -> Result<()>
    where
        N: Into<Vec<u8>>,
        S: Into<Vec<u8>>,
    {
        unsafe {
            let _ = Self::unsafe_declare(ctx, name, source)?;
        }
        Ok(())
    }

    /// Creates a new module from JS source, and evaluates it.
    ///
    /// It is unsafe to hold onto unevaluated modules across this call.
    pub fn evaluate<N, S>(ctx: Ctx<'js>, name: N, source: S) -> Result<Module<'js>>
    where
        N: Into<Vec<u8>>,
        S: Into<Vec<u8>>,
    {
        let module = unsafe { Self::unsafe_declare(ctx, name, source)? };
        unsafe { module.eval()? };
        Ok(module)
    }

    /// Declare a module in the runtime.
    ///
    /// This function doesn't return a module since holding on to unevaluated modules is unsafe.
    /// If you need to hold onto unsafe modules use the [`Module::unsafe_declare_def`] functions.
    ///
    /// It is unsound to hold onto an unevaluated module across any call to this function which
    /// returns an error.
    pub fn declare_def<D, N>(ctx: Ctx<'js>, name: N) -> Result<()>
    where
        N: Into<Vec<u8>>,
        D: ModuleDef,
    {
        unsafe {
            let _ = Self::unsafe_declare_def::<D, _>(ctx, name)?;
        }
        Ok(())
    }

    /// Declares a module in the runtime and evaluates it.
    ///
    /// It is unsound to hold onto an unevaluated module across any call to this function which
    /// returns an error.
    pub fn evaluate_def<D, N>(ctx: Ctx<'js>, name: N) -> Result<Module<'js>>
    where
        N: Into<Vec<u8>>,
        D: ModuleDef,
    {
        let module = unsafe { Self::unsafe_declare_def::<D, _>(ctx, name)? };
        unsafe { module.eval()? };
        Ok(module)
    }

    /// Returns the name of the module
    pub fn name<N>(&self) -> Result<N>
    where
        N: FromAtom<'js>,
    {
        let name = unsafe {
            Atom::from_atom_val(
                self.ctx.clone(),
                qjs::JS_GetModuleName(self.ctx.as_ptr(), self.as_module_def().as_ptr()),
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
                self.ctx.handle_exception(qjs::JS_GetImportMeta(
                    self.ctx.as_ptr(),
                    self.as_module_def().as_ptr(),
                ))?,
            )
        };
        T::from_js(&self.ctx, meta)
    }

    /// Write object bytecode for the module in little endian format.
    pub fn write_object_le(&self) -> Result<Vec<u8>> {
        let swap = cfg!(target_endian = "big");
        self.write_object(swap)
    }

    /// Write object bytecode for the module in big endian format.
    pub fn write_object_be(&self) -> Result<Vec<u8>> {
        let swap = cfg!(target_endian = "little");
        self.write_object(swap)
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
        match Self::unsafe_declare_def::<D, _>(ctx.clone(), name) {
            Ok(module) => module.as_module_def().as_ptr(),
            Err(error) => {
                error.throw(&ctx);
                ptr::null_mut()
            }
        }
    }

    /// Write object bytecode for the module.
    ///
    /// `swap_endianess` swaps the endianness of the bytecode, if true, from native to the other
    /// kind. Use if the bytecode is meant for a target with a different endianness than the
    /// current.
    pub fn write_object(&self, swap_endianess: bool) -> Result<Vec<u8>> {
        let ctx = &self.ctx;
        let mut len = MaybeUninit::uninit();
        // TODO: Allow inclusion of other flags?
        let mut flags = qjs::JS_WRITE_OBJ_BYTECODE;
        if swap_endianess {
            flags |= qjs::JS_WRITE_OBJ_BSWAP;
        }
        let value = qjs::JS_MKPTR(qjs::JS_TAG_MODULE, self.module.as_ptr().cast());
        let buf =
            unsafe { qjs::JS_WriteObject(ctx.as_ptr(), len.as_mut_ptr(), value, flags as i32) };
        if buf.is_null() {
            return Err(ctx.raise_exception());
        }
        let len = unsafe { len.assume_init() };
        let obj = unsafe { slice::from_raw_parts(buf, len as _) };
        let obj = Vec::from(obj);
        unsafe { qjs::js_free(ctx.as_ptr(), buf as _) };
        Ok(obj)
    }

    /// Read object bytecode and declare it as a module.
    pub fn declare_read_object(ctx: Ctx<'js>, bytes: &[u8]) -> Result<()> {
        unsafe {
            let _ = Self::unsafe_declare_read_object(ctx, bytes)?;
        }
        Ok(())
    }

    /// Read object bytecode and declare it as a module and then evaluate it.
    pub fn instantiate_read_object(ctx: Ctx<'js>, bytes: &[u8]) -> Result<Module<'js>> {
        let module = unsafe { Self::unsafe_declare_read_object(ctx, bytes)? };
        unsafe {
            module.eval()?;
        }
        Ok(module)
    }

    /// Read object bytecode and declare it as a module.
    ///
    /// # Safety
    ///
    /// QuickJS frees all unevaluated modules if any error happens while declaring or evaluating a
    /// module. If any call to either `Module::new` or `Module::eval` fails it is undefined
    /// behavior to use any unevaluated modules.
    ///
    /// It is unsound to hold onto an unevaluated module across any call to this function which
    /// returns an error.
    pub unsafe fn unsafe_declare_read_object(ctx: Ctx<'js>, bytes: &[u8]) -> Result<Module<'js>> {
        let module = unsafe {
            qjs::JS_ReadObject(
                ctx.as_ptr(),
                bytes.as_ptr(),
                bytes.len() as _,
                (qjs::JS_READ_OBJ_BYTECODE | qjs::JS_READ_OBJ_ROM_DATA) as i32,
            )
        };
        let module = ctx.handle_exception(module)?;
        debug_assert_eq!(qjs::JS_TAG_MODULE, unsafe { qjs::JS_VALUE_GET_TAG(module) });
        let module = qjs::JS_VALUE_GET_PTR(module).cast::<qjs::JSModuleDef>();
        // QuickJS should throw an exception on allocation errors
        // So this should always be non-null.
        let module = NonNull::new(module).unwrap();
        Ok(Module { ctx, module })
    }

    /// Creates a new module from JS source but doesn't evaluate the module.
    ///
    /// # Safety
    ///
    /// QuickJS frees all unevaluated modules if any error happens while declaring or evaluating a
    /// module. If any call to either `Module::new` or `Module::eval` fails it is undefined
    /// behavior to use any unevaluated modules.
    ///
    /// It is unsound to hold onto an unevaluated module across any call to this function which
    /// returns an error.
    pub unsafe fn unsafe_declare<N, S>(ctx: Ctx<'js>, name: N, source: S) -> Result<Module<'js>>
    where
        N: Into<Vec<u8>>,
        S: Into<Vec<u8>>,
    {
        let name = CString::new(name)?;
        let flag =
            qjs::JS_EVAL_TYPE_MODULE | qjs::JS_EVAL_FLAG_STRICT | qjs::JS_EVAL_FLAG_COMPILE_ONLY;

        let module = unsafe { ctx.eval_raw(source, name.as_c_str(), flag as i32)? };
        let module = ctx.handle_exception(module)?;
        debug_assert_eq!(qjs::JS_TAG_MODULE, unsafe { qjs::JS_VALUE_GET_TAG(module) });
        let module = qjs::JS_VALUE_GET_PTR(module).cast::<qjs::JSModuleDef>();
        // QuickJS should throw an exception on allocation errors
        // So this should always be non-null.
        let module = NonNull::new(module).unwrap();

        Ok(Module { ctx, module })
    }

    /// Creates a new module from JS source but doesn't evaluate the module.
    ///
    /// # Safety
    /// It is unsafe to use an unevaluated for anything other then evaluating it with
    /// `Module::eval`.
    ///
    /// QuickJS frees all unevaluated modules if any error happens while declaring or evaluating a
    /// module. If any call to either `Module::new` or `Module::eval` fails it is undefined
    /// behavior to use any unevaluated modules.
    ///
    /// It is unsound to hold onto an unevaluated module across any call to this function which
    /// returns an error.
    pub unsafe fn unsafe_declare_def<D, N>(ctx: Ctx<'js>, name: N) -> Result<Module<'js>>
    where
        N: Into<Vec<u8>>,
        D: ModuleDef,
    {
        let name = CString::new(name)?;

        let mut defs = Declarations::new();
        D::declare(&mut defs)?;

        let ptr =
            unsafe { qjs::JS_NewCModule(ctx.as_ptr(), name.as_ptr(), Some(Self::eval_fn::<D>)) };

        let ptr = NonNull::new(ptr).ok_or(Error::Allocation)?;
        let module = Module::from_module_def(ctx.clone(), ptr);
        // Safety: Safe because this is a newly created
        unsafe { defs.apply(ctx, &module)? };
        Ok(module)
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
        let ptr = NonNull::new_unchecked(ptr);
        let module = Self::from_module_def(ctx.clone(), ptr);
        let mut exports = Exports::new(ctx.clone());
        match D::evaluate(&ctx, &mut exports).and_then(|_| exports.apply(module)) {
            Ok(_) => 0,
            Err(error) => {
                error.throw(&ctx);
                -1
            }
        }
    }

    /// Evaluates an unevaluated module.
    ///
    /// # Safety
    /// This function should only be called on unevaluated modules.
    ///
    /// QuickJS frees all unevaluated modules if any error happens while declaring or evaluating a
    /// module. If any call to either `Module::new` or `Module::eval` fails it is undefined
    /// behavior to use any unevaluated modules.
    ///
    /// Prefer the use of either `ModuleBuilder` or `Module::new`.
    ///
    /// It is unsound to hold onto an unevaluated module across any call to this function which
    /// returns an error.
    pub unsafe fn eval(&self) -> Result<()> {
        unsafe {
            let value = qjs::JS_MKPTR(qjs::JS_TAG_MODULE, self.module.as_ptr().cast());
            // JS_EvalFunction `free's` the module so we should dup first
            let ret = qjs::JS_EvalFunction(self.ctx.as_ptr(), qjs::JS_DupValue(value));
            self.ctx.handle_exception(ret)?;
        }
        Ok(())
    }

    /// Import and evaluate a module
    ///
    /// This will work similar to an `await import(specifier)` statement in JavaScript but will return the import and not a promise
    pub fn import<V: FromJs<'js>, S: AsRef<[u8]>>(ctx: &Ctx<'js>, specifier: S) -> Result<V> {
        let specifier = specifier.as_ref();
        let val = unsafe {
            let js_string_val = qjs::JS_NewStringLen(
                ctx.as_ptr(),
                specifier.as_ptr() as *mut _,
                specifier.len() as qjs::size_t,
            );
            let js_string = qjs::JS_ToCString(ctx.as_ptr(), js_string_val);
            let val = qjs::JS_DynamicImportSync(ctx.as_ptr(), js_string);
            qjs::JS_FreeValue(ctx.as_ptr(), js_string_val);
            let val = ctx.handle_exception(val)?;
            Value::from_js_value(ctx.clone(), val)
        };

        V::from_js(ctx, val)
    }
}

#[cfg(feature = "exports")]
impl<'js> Module<'js> {
    /// Return exported value by name
    pub fn get<N, T>(&self, name: N) -> Result<T>
    where
        N: AsRef<str>,
        T: FromJs<'js>,
    {
        let name = CString::new(name.as_ref())?;
        let value = unsafe {
            Value::from_js_value(
                self.ctx.clone(),
                self.ctx.handle_exception(qjs::JS_GetModuleExport(
                    self.ctx.as_ptr(),
                    self.as_module_def().as_ptr(),
                    name.as_ptr(),
                ))?,
            )
        };
        T::from_js(&self.ctx, value)
    }

    /// Returns a iterator over the exported names of the module export.
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "exports")))]
    pub fn names<N>(self) -> ExportNamesIter<'js, N>
    where
        N: FromAtom<'js>,
    {
        let count = unsafe { qjs::JS_GetModuleExportEntriesCount(self.as_module_def().as_ptr()) };
        ExportNamesIter {
            module: self,
            count,
            index: 0,
            marker: PhantomData,
        }
    }

    /// Returns a iterator over the items the module export.
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "exports")))]
    pub fn entries<N, T>(self) -> ExportEntriesIter<'js, N, T>
    where
        N: FromAtom<'js>,
        T: FromJs<'js>,
    {
        let count = unsafe { qjs::JS_GetModuleExportEntriesCount(self.as_module_def().as_ptr()) };
        ExportEntriesIter {
            module: self,
            count,
            index: 0,
            marker: PhantomData,
        }
    }

    #[doc(hidden)]
    pub unsafe fn dump_exports(&self) {
        let ptr = self.as_module_def().as_ptr();
        let count = qjs::JS_GetModuleExportEntriesCount(ptr);
        for i in 0..count {
            let atom_name = Atom::from_atom_val(
                self.ctx.clone(),
                qjs::JS_GetModuleExportEntryName(self.ctx.as_ptr(), ptr, i),
            );
            println!("{}", atom_name.to_string().unwrap());
        }
    }
}

/// An iterator over the items exported out a module
#[cfg(feature = "exports")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "exports")))]
pub struct ExportNamesIter<'js, N> {
    module: Module<'js>,
    count: i32,
    index: i32,
    marker: PhantomData<N>,
}

#[cfg(feature = "exports")]
impl<'js, N> Iterator for ExportNamesIter<'js, N>
where
    N: FromAtom<'js>,
{
    type Item = Result<N>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.count {
            return None;
        }
        let ctx = &self.module.ctx;
        let ptr = self.module.as_module_def().as_ptr();
        let atom = unsafe {
            let atom_val = qjs::JS_GetModuleExportEntryName(ctx.as_ptr(), ptr, self.index);
            Atom::from_atom_val(ctx.clone(), atom_val)
        };
        self.index += 1;
        Some(N::from_atom(atom))
    }
}

/// An iterator over the items exported out a module
#[cfg(feature = "exports")]
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "exports")))]
pub struct ExportEntriesIter<'js, N, T> {
    module: Module<'js>,
    count: i32,
    index: i32,
    marker: PhantomData<(N, T)>,
}

#[cfg(feature = "exports")]
impl<'js, N, T> Iterator for ExportEntriesIter<'js, N, T>
where
    N: FromAtom<'js>,
    T: FromJs<'js>,
{
    type Item = Result<(N, T)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.count {
            return None;
        }
        let ctx = &self.module.ctx;
        let ptr = self.module.as_module_def().as_ptr();
        let name = unsafe {
            let atom_val = qjs::JS_GetModuleExportEntryName(ctx.as_ptr(), ptr, self.index);
            Atom::from_atom_val(ctx.clone(), atom_val)
        };
        let value = unsafe {
            let js_val = qjs::JS_GetModuleExportEntry(ctx.as_ptr(), ptr, self.index);
            Value::from_js_value(ctx.clone(), js_val)
        };
        self.index += 1;
        Some(N::from_atom(name).and_then(|name| T::from_js(ctx, value).map(|value| (name, value))))
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::*;

    pub struct RustModule;

    impl ModuleDef for RustModule {
        fn declare(define: &mut Declarations) -> Result<()> {
            define.declare_static(CStr::from_bytes_with_nul(b"hello\0")?)?;
            Ok(())
        }

        fn evaluate<'js>(_ctx: &Ctx<'js>, exports: &mut Exports<'js>) -> Result<()> {
            exports.export("hello", "world".to_string())?;
            Ok(())
        }
    }

    pub struct CrashingRustModule;

    impl ModuleDef for CrashingRustModule {
        fn declare(_: &mut Declarations) -> Result<()> {
            Ok(())
        }

        fn evaluate<'js>(ctx: &Ctx<'js>, _exports: &mut Exports<'js>) -> Result<()> {
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
            let _ = ctx
                .clone()
                .compile(
                    "test",
                    r#"
                import { hello } from "rust_mod";

                globalThis.hello = hello;
            "#,
                )
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
            let val: Object = Module::import(&ctx, "rust_mod").unwrap();
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
            let _: Value = Module::import(&ctx, "bad_rust_mod").catch(&ctx).unwrap();
        });
    }

    #[test]
    fn holding_onto_unevaluated() {
        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let module = unsafe {
                Module::unsafe_declare(
                    ctx.clone(),
                    "test",
                    "export function add(a,b){ return a + b }",
                )
                .unwrap()
            };
            // Error
            ctx.compile("test2", "throw new Error(1)").ok();

            unsafe { module.eval().unwrap() }
        });
    }

    #[test]
    fn eval_crashing_module_inside_module() {
        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();

        ctx.with(|ctx| {
            let globals = ctx.globals();
            let eval_crashing = |ctx: Ctx| ctx.compile("test2", "throw new Error(1)").map(|_| ());
            let function = Function::new(ctx.clone(), eval_crashing).unwrap();
            globals.set("eval_crashing", function).unwrap();

            let res = ctx.compile("test", " eval_crashing(); ");
            assert!(res.is_err())
        });
    }

    #[test]
    fn from_javascript() {
        test_with(|ctx| {
            let module: Module = ctx
                .clone()
                .compile(
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

            assert_eq!(module.name::<StdString>().unwrap(), "Test");
            let _ = module.meta::<Object>().unwrap();

            #[cfg(feature = "exports")]
            {
                let names = module
                    .clone()
                    .names()
                    .collect::<Result<Vec<StdString>>>()
                    .unwrap();

                assert_eq!(names[0], "a");
                assert_eq!(names[1], "foo");
                assert_eq!(names[2], "Baz");

                let entries = module
                    .clone()
                    .entries()
                    .collect::<Result<Vec<(StdString, Value)>>>()
                    .unwrap();

                assert_eq!(entries[0].0, "a");
                assert_eq!(i32::from_js(&ctx, entries[0].1.clone()).unwrap(), 2);
                assert_eq!(entries[1].0, "foo");
                assert_eq!(entries[2].0, "Baz");
            }
        });
    }
}
