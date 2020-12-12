use crate::{
    get_exception, handle_exception, qjs, Atom, Ctx, Error, FromAtom, FromJs, IntoJs, Result, Value,
};
use std::{
    ffi::{CStr, CString},
    marker::PhantomData,
    ptr,
};

/// The marker for the module which is created from text source
pub struct Script;

/// The marker for the module which is created using `ModuleDef`
pub struct Native;

/// The marker for the module which is created but not loaded yet
pub struct Created;

/// The marker for the module which is loaded but not evaluated yet
pub struct Loaded<S = ()>(S);

/// The marker for the module which is already loaded and evaluated
pub struct Evaluated;

/// Module definition trait
pub trait ModuleDef {
    /// The exports should be added here
    fn load<'js>(_ctx: Ctx<'js>, _module: &Module<'js, Created>) -> Result<()> {
        Ok(())
    }

    /// The exports should be set here
    fn eval<'js>(_ctx: Ctx<'js>, _module: &Module<'js, Loaded<Native>>) -> Result<()> {
        Ok(())
    }
}

/// Javascript module with certain exports and imports
#[derive(Debug, PartialEq)]
pub struct Module<'js, S = Evaluated>(pub(crate) Value<'js>, pub(crate) PhantomData<S>);

impl<'js, S> Clone for Module<'js, S> {
    fn clone(&self) -> Self {
        Module(self.0.clone(), PhantomData)
    }
}

impl<'js, S> Module<'js, S> {
    pub(crate) unsafe fn from_module_def(ctx: Ctx<'js>, ptr: *mut qjs::JSModuleDef) -> Self {
        Self(
            Value::new_ptr(ctx, qjs::JS_TAG_MODULE, ptr as _),
            PhantomData,
        )
    }

    pub(crate) unsafe fn from_module_def_const(ctx: Ctx<'js>, ptr: *mut qjs::JSModuleDef) -> Self {
        Self(
            Value::new_ptr_const(ctx, qjs::JS_TAG_MODULE, ptr as _),
            PhantomData,
        )
    }

    pub(crate) fn as_module_def(&self) -> *mut qjs::JSModuleDef {
        unsafe { self.0.get_ptr() as _ }
    }

    pub(crate) fn into_module_def(self) -> *mut qjs::JSModuleDef {
        unsafe { self.0.into_ptr() as _ }
    }
}

impl<'js> Module<'js> {
    /// Returns the name of the module
    pub fn name<N>(&self) -> Result<N>
    where
        N: FromAtom<'js>,
    {
        let ctx = self.0.ctx;
        let name = unsafe {
            Atom::from_atom_val(ctx, qjs::JS_GetModuleName(ctx.ctx, self.as_module_def()))
        };
        N::from_atom(name)
    }

    /// Return the `import.meta` object of a module
    pub fn meta<T>(&self) -> Result<T>
    where
        T: FromJs<'js>,
    {
        let ctx = self.0.ctx;
        let meta = unsafe {
            Value::from_js_value(
                ctx,
                handle_exception(ctx, qjs::JS_GetImportMeta(ctx.ctx, self.as_module_def()))?,
            )
        };
        T::from_js(ctx, meta)
    }
}

/// Helper macro to provide module init function
///
/// ```
/// use rquickjs::{ModuleDef, module_init};
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
            $crate::Function::init_raw(ctx);
            $crate::Module::init::<$type>(ctx, module_name)
        }
    };
}

/// The raw module load function (`js_module_init`)
pub type ModuleLoadFn =
    unsafe extern "C" fn(*mut qjs::JSContext, *const qjs::c_char) -> *mut qjs::JSModuleDef;

impl<'js> Module<'js> {
    /// Create module from JS source
    #[allow(clippy::new_ret_no_self)]
    pub fn new<N, S>(ctx: Ctx<'js>, name: N, source: S) -> Result<Module<'js, Loaded<Script>>>
    where
        N: Into<Vec<u8>>,
        S: Into<Vec<u8>>,
    {
        let name = CString::new(name)?;
        let flag =
            qjs::JS_EVAL_TYPE_MODULE | qjs::JS_EVAL_FLAG_STRICT | qjs::JS_EVAL_FLAG_COMPILE_ONLY;
        Ok(Module(
            unsafe {
                let value = Value::from_js_value_const(
                    ctx,
                    ctx.eval_raw(source, name.as_c_str(), flag as _)?,
                );
                debug_assert!(value.is_module());
                value
            },
            PhantomData,
        ))
    }

    /// Create native JS module using [`ModuleDef`]
    #[allow(clippy::new_ret_no_self)]
    pub fn new_def<D, N>(ctx: Ctx<'js>, name: N) -> Result<Module<'js, Loaded<Native>>>
    where
        D: ModuleDef,
        N: Into<Vec<u8>>,
    {
        let name = CString::new(name)?;
        let ptr = unsafe {
            qjs::JS_NewCModule(
                ctx.ctx,
                name.as_ptr(),
                Some(Module::<Loaded<Native>>::eval_fn::<D>),
            )
        };
        if ptr.is_null() {
            return Err(Error::Allocation);
        }
        let module = unsafe { Module::<Created>::from_module_def(ctx, ptr) };
        D::load(ctx, &module)?;
        Ok(Module(module.0, PhantomData))
    }

    /// Create native JS module by calling init function (like `js_module_init`)
    ///
    /// # Safety
    /// The `load` function should not crash. But it can throw exception and return null pointer in that case.
    #[allow(clippy::new_ret_no_self)]
    pub unsafe fn new_raw<N>(
        ctx: Ctx<'js>,
        name: N,
        load: ModuleLoadFn,
    ) -> Result<Module<'js, Loaded<Native>>>
    where
        N: Into<Vec<u8>>,
    {
        let name = CString::new(name)?;
        let ptr = load(ctx.ctx, name.as_ptr());

        if ptr.is_null() {
            Err(Error::Unknown)
        } else {
            Ok(Module::from_module_def(ctx, ptr))
        }
    }

    /// The function for loading native JS module
    ///
    /// # Safety
    /// This function should only be called from `js_module_init` function.
    pub unsafe extern "C" fn init<D>(
        ctx: *mut qjs::JSContext,
        name: *const qjs::c_char,
    ) -> *mut qjs::JSModuleDef
    where
        D: ModuleDef,
    {
        let ctx = Ctx::from_ptr(ctx);
        let name = CStr::from_ptr(name);
        match Self::_init::<D>(ctx, name) {
            Ok(module) => module.into_module_def(),
            Err(error) => {
                error.throw(ctx);
                ptr::null_mut() as _
            }
        }
    }

    fn _init<D>(ctx: Ctx<'js>, name: &CStr) -> Result<Module<'js, Loaded>>
    where
        D: ModuleDef,
    {
        let name = name.to_str()?;
        Ok(Module::new_def::<D, _>(ctx, name)?.into_loaded())
    }
}

impl<'js> Module<'js, Loaded<Native>> {
    /// Set exported entry by name
    ///
    /// NOTE: Exported entries should be added before module instantiating using [Module::add].
    pub fn set<N, T>(&self, name: N, value: T) -> Result<()>
    where
        N: AsRef<str>,
        T: IntoJs<'js>,
    {
        let name = CString::new(name.as_ref())?;
        let ctx = self.0.ctx;
        let value = value.into_js(ctx)?;
        let value = unsafe { qjs::JS_DupValue(value.as_js_value()) };
        if unsafe { qjs::JS_SetModuleExport(ctx.ctx, self.as_module_def(), name.as_ptr(), value) }
            < 0
        {
            unsafe { qjs::JS_FreeValue(ctx.ctx, value) };
            return Err(unsafe { get_exception(ctx) });
        }
        Ok(())
    }

    unsafe extern "C" fn eval_fn<D>(
        ctx: *mut qjs::JSContext,
        ptr: *mut qjs::JSModuleDef,
    ) -> qjs::c_int
    where
        D: ModuleDef,
    {
        let ctx = Ctx::from_ptr(ctx);
        let module = Self::from_module_def_const(ctx, ptr);
        match D::eval(ctx, &module) {
            Ok(_) => 0,
            Err(error) => {
                error.throw(ctx);
                -1
            }
        }
    }
}

impl<'js, S> Module<'js, Loaded<S>> {
    /// Evaluate a loaded module
    ///
    /// To get access to module exports it should be evaluated first, in particular when you create module manually via [`Module::new`].
    pub fn eval(self) -> Result<Module<'js, Evaluated>> {
        let ctx = self.0.ctx;
        unsafe {
            let ret = qjs::JS_EvalFunction(ctx.ctx, self.0.value);
            handle_exception(ctx, ret)?;
        }
        Ok(Module(self.0, PhantomData))
    }

    /// Cast the specific loaded module to generic one
    pub fn into_loaded(self) -> Module<'js, Loaded> {
        Module(self.0, PhantomData)
    }
}

impl<'js> Module<'js, Created> {
    /// Add entry to module exports
    ///
    /// NOTE: Added entries should be set after module instantiating using [Module::set].
    pub fn add<N>(&self, name: N) -> Result<()>
    where
        N: AsRef<str>,
    {
        let ctx = self.0.ctx;
        let name = CString::new(name.as_ref())?;
        unsafe {
            qjs::JS_AddModuleExport(ctx.ctx, self.as_module_def(), name.as_ptr());
        }
        Ok(())
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
        let ctx = self.0.ctx;
        let name = CString::new(name.as_ref())?;
        let value = unsafe {
            Value::from_js_value(
                ctx,
                handle_exception(
                    ctx,
                    qjs::JS_GetModuleExport(ctx.ctx, self.as_module_def(), name.as_ptr()),
                )?,
            )
        };
        T::from_js(ctx, value)
    }

    /// Returns a iterator over the exported names of the module export.
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "exports")))]
    pub fn names<N>(&self) -> ExportNamesIter<'js, N>
    where
        N: FromAtom<'js>,
    {
        ExportNamesIter {
            module: self.clone(),
            count: unsafe { qjs::JS_GetModuleExportEntriesCount(self.as_module_def()) },
            index: 0,
            marker: PhantomData,
        }
    }

    /// Returns a iterator over the items the module export.
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "exports")))]
    pub fn entries<N, T>(&self) -> ExportEntriesIter<'js, N, T>
    where
        N: FromAtom<'js>,
        T: FromJs<'js>,
    {
        ExportEntriesIter {
            module: self.clone(),
            count: unsafe { qjs::JS_GetModuleExportEntriesCount(self.as_module_def()) },
            index: 0,
            marker: PhantomData,
        }
    }

    #[doc(hidden)]
    pub unsafe fn dump_exports(&self) {
        let ctx = self.0.ctx;
        let ptr = self.as_module_def();
        let count = qjs::JS_GetModuleExportEntriesCount(ptr);
        for i in 0..count {
            let atom_name =
                Atom::from_atom_val(ctx, qjs::JS_GetModuleExportEntryName(ctx.ctx, ptr, i));
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
        let ctx = self.module.0.ctx;
        let ptr = self.module.as_module_def();
        let atom = unsafe {
            let atom_val = qjs::JS_GetModuleExportEntryName(ctx.ctx, ptr, self.index);
            Atom::from_atom_val(ctx, atom_val)
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
        let ctx = self.module.0.ctx;
        let ptr = self.module.as_module_def();
        let name = unsafe {
            let atom_val = qjs::JS_GetModuleExportEntryName(ctx.ctx, ptr, self.index);
            Atom::from_atom_val(ctx, atom_val)
        };
        let value = unsafe {
            let js_val = qjs::JS_GetModuleExportEntry(ctx.ctx, ptr, self.index);
            Value::from_js_value(ctx, js_val)
        };
        self.index += 1;
        Some(N::from_atom(name).and_then(|name| T::from_js(ctx, value).map(|value| (name, value))))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::*;

    #[test]
    fn from_javascript() {
        test_with(|ctx| {
            let module: Module = ctx
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
                let names = module.names().collect::<Result<Vec<StdString>>>().unwrap();

                assert_eq!(names[0], "a");
                assert_eq!(names[1], "foo");
                assert_eq!(names[2], "Baz");

                let entries = module
                    .entries()
                    .collect::<Result<Vec<(StdString, Value)>>>()
                    .unwrap();

                assert_eq!(entries[0].0, "a");
                assert_eq!(i32::from_js(ctx, entries[0].1.clone()).unwrap(), 2);
                assert_eq!(entries[1].0, "foo");
                assert_eq!(entries[2].0, "Baz");
            }
        });
    }
}
