use crate::{qjs, value, Atom, Ctx, Error, FromAtom, FromJs, IntoJs, Result, Value};
use std::{
    ffi::{CStr, CString},
    marker::PhantomData,
    ptr,
};

/// Module definition trait
pub trait ModuleDef {
    /// The exports should be added here
    fn before_init<'js>(_ctx: Ctx<'js>, _module: &Module<'js, BeforeInit>) -> Result<()> {
        Ok(())
    }

    /// The exports should be set here
    fn after_init<'js>(_ctx: Ctx<'js>, _module: &Module<'js, AfterInit>) -> Result<()> {
        Ok(())
    }
}

/// Marker for the module which is not instantiated yet
pub struct BeforeInit;

/// Marker for the module which is already instantiated
pub struct AfterInit;

/// Javascript module with certain exports and imports
#[derive(Debug)]
pub struct Module<'js, S = AfterInit> {
    ptr: *mut qjs::JSModuleDef,
    ctx: Ctx<'js>,
    marker: PhantomData<S>,
}

impl<'js, S> Clone for Module<'js, S> {
    fn clone(&self) -> Self {
        Module {
            ptr: self.ptr,
            ctx: self.ctx,
            marker: PhantomData,
        }
    }
}

impl<'js, S> PartialEq<Module<'js, S>> for Module<'js, S> {
    fn eq(&self, other: &Module<'js, S>) -> bool {
        self.ptr == other.ptr
    }
}

impl<'js, S> Module<'js, S> {
    pub(crate) unsafe fn from_module_def(ctx: Ctx<'js>, ptr: *mut qjs::JSModuleDef) -> Self {
        Self {
            ptr,
            ctx,
            marker: PhantomData,
        }
    }

    pub(crate) fn as_module_def(&self) -> *mut qjs::JSModuleDef {
        self.ptr
    }

    pub(crate) unsafe fn from_js_value(ctx: Ctx<'js>, js_val: qjs::JSValue) -> Self {
        debug_assert_eq!(qjs::JS_VALUE_GET_NORM_TAG(js_val), qjs::JS_TAG_MODULE);
        let ptr = qjs::JS_VALUE_GET_PTR(js_val) as _;
        Self::from_module_def(ctx, ptr)
    }

    #[allow(dead_code)]
    pub(crate) fn as_js_value(&self) -> qjs::JSValue {
        qjs::JS_MKPTR(qjs::JS_TAG_MODULE, self.ptr as *mut _)
    }
}

impl<'js> Module<'js> {
    /// Returns the name of the module
    pub fn name<N>(&self) -> Result<N>
    where
        N: FromAtom<'js>,
    {
        let name =
            unsafe { Atom::from_atom_val(self.ctx, qjs::JS_GetModuleName(self.ctx.ctx, self.ptr)) };
        N::from_atom(name)
    }

    /// Return the `import.meta` object of a module
    pub fn meta<T>(&self) -> Result<T>
    where
        T: FromJs<'js>,
    {
        let meta = unsafe {
            Value::from_js_value(self.ctx, qjs::JS_GetImportMeta(self.ctx.ctx, self.ptr))
        }?;
        T::from_js(self.ctx, meta)
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
            $crate::Module::init::<$type>(ctx, module_name)
        }
    };
}

impl<'js> Module<'js> {
    /// Create native JS module
    pub fn new<D, N>(ctx: Ctx<'js>, name: N) -> Result<Module<'js, BeforeInit>>
    where
        D: ModuleDef,
        N: AsRef<str>,
    {
        let name = CString::new(name.as_ref())?;
        let ptr = unsafe {
            qjs::JS_NewCModule(
                ctx.ctx,
                name.as_ptr(),
                Some(Module::<BeforeInit>::init_fn::<D>),
            )
        };
        if ptr.is_null() {
            return Err(Error::Allocation);
        }
        let module = unsafe { Module::<BeforeInit>::from_module_def(ctx, ptr) };
        D::before_init(ctx, &module)?;
        Ok(module)
    }

    /// The function for loading native JS module
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
            Ok(module) => module.as_module_def(),
            Err(error) => {
                error.throw(ctx);
                ptr::null_mut() as _
            }
        }
    }

    fn _init<D>(ctx: Ctx<'js>, name: &CStr) -> Result<Module<'js, BeforeInit>>
    where
        D: ModuleDef,
    {
        let name = name.to_str()?;
        Module::new::<D, _>(ctx, name)
    }

    /// Set exported entry by name
    ///
    /// NOTE: Exported entries should be added before module instantiating using [Module::add].
    pub fn set<N, T>(&self, name: N, value: T) -> Result<()>
    where
        N: AsRef<str>,
        T: IntoJs<'js>,
    {
        let name = CString::new(name.as_ref())?;
        let value = value.into_js(self.ctx)?;
        let value = unsafe { qjs::JS_DupValue(value.as_js_value()) };
        if unsafe { qjs::JS_SetModuleExport(self.ctx.ctx, self.ptr, name.as_ptr(), value) } < 0 {
            unsafe { qjs::JS_FreeValue(self.ctx.ctx, value) };
            return Err(unsafe { value::get_exception(self.ctx) });
        }
        Ok(())
    }
}

impl<'js> Module<'js, BeforeInit> {
    unsafe extern "C" fn init_fn<D>(
        ctx: *mut qjs::JSContext,
        ptr: *mut qjs::JSModuleDef,
    ) -> qjs::c_int
    where
        D: ModuleDef,
    {
        let ctx = Ctx::from_ptr(ctx);
        let module = Module::<AfterInit>::from_module_def(ctx, ptr);
        match D::after_init(ctx, &module) {
            Ok(_) => 0,
            Err(error) => {
                error.throw(ctx);
                -1
            }
        }
    }

    /// Add entry to module exports
    ///
    /// NOTE: Added entries should be set after module instantiating using [Module::set].
    pub fn add<N>(&self, name: N) -> Result<()>
    where
        N: AsRef<str>,
    {
        let name = CString::new(name.as_ref())?;
        unsafe {
            qjs::JS_AddModuleExport(self.ctx.ctx, self.ptr, name.as_ptr());
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
        let name = CString::new(name.as_ref())?;
        let value = unsafe {
            Value::from_js_value(
                self.ctx,
                qjs::JS_GetModuleExport(self.ctx.ctx, self.ptr, name.as_ptr()),
            )
        }?;
        T::from_js(self.ctx, value)
    }

    /// Returns a iterator over the exported names of the module export.
    ///
    /// # Features
    /// This function is only availble if the `exports` feature is enabled.
    pub fn names<N>(&self) -> ExportNamesIter<'js, N>
    where
        N: FromAtom<'js>,
    {
        ExportNamesIter {
            module: self.clone(),
            count: unsafe { qjs::JS_GetModuleExportEntriesCount(self.ptr) },
            index: 0,
            marker: PhantomData,
        }
    }

    /// Returns a iterator over the items the module export.
    ///
    /// # Features
    /// This function is only availble if the `exports` feature is enabled.
    pub fn entries<N, T>(&self) -> ExportEntriesIter<'js, N, T>
    where
        N: FromAtom<'js>,
        T: FromJs<'js>,
    {
        ExportEntriesIter {
            module: self.clone(),
            count: unsafe { qjs::JS_GetModuleExportEntriesCount(self.ptr) },
            index: 0,
            marker: PhantomData,
        }
    }

    #[doc(hidden)]
    pub unsafe fn dump_exports(&self) {
        let ptr = self.ptr;
        let count = qjs::JS_GetModuleExportEntriesCount(ptr);
        for i in 0..count {
            let atom_name = Atom::from_atom_val(
                self.ctx,
                qjs::JS_GetModuleExportEntryName(self.ctx.ctx, ptr, i),
            );
            println!("{}", atom_name.to_string().unwrap());
        }
    }
}

/// An iterator over the items exported out a module
///
/// # Features
/// This struct is only availble if the `exports` feature is enabled.
#[cfg(feature = "exports")]
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
        let ctx = self.module.ctx;
        let ptr = self.module.ptr;
        let atom = unsafe {
            let atom_val = qjs::JS_GetModuleExportEntryName(ctx.ctx, ptr, self.index);
            Atom::from_atom_val(ctx, atom_val)
        };
        self.index += 1;
        Some(N::from_atom(atom))
    }
}

/// An iterator over the items exported out a module
///
/// # Features
/// This struct is only availble if the `exports` feature is enabled.
#[cfg(feature = "exports")]
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
        let ctx = self.module.ctx;
        let ptr = self.module.ptr;
        let name = unsafe {
            let atom_val = qjs::JS_GetModuleExportEntryName(ctx.ctx, ptr, self.index);
            Atom::from_atom_val(ctx, atom_val)
        };
        let value = unsafe {
            let js_val = qjs::JS_GetModuleExportEntry(ctx.ctx, ptr, self.index);
            Value::from_js_value(ctx, js_val).unwrap()
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
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
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
                assert_eq!(entries[0].1, Value::Int(2));
                assert_eq!(entries[1].0, "foo");
                assert_eq!(entries[2].0, "Baz");
            }
        });
    }
}
