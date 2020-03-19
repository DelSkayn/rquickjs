use crate::Ctx;
#[cfg(feature = "exports")]
use crate::{Atom, Value};
use rquickjs_sys as qjs;
use std::ffi::c_void;

#[cfg(feature = "exports")]
pub struct ExportList<'js> {
    m: Module<'js>,
    max: i32,
    cur: i32,
}

#[cfg(feature = "exports")]
impl<'js> Iterator for ExportList<'js> {
    type Item = (Atom<'js>, Value<'js>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur == self.max {
            return None;
        }
        let ptr = self.m.ptr as *mut qjs::JSModuleDef;
        let atom = unsafe {
            let atom_val = qjs::JS_GetModuleExportEntryName(self.m.ctx.ctx, ptr, self.cur);
            Atom::from_atom_val(self.m.ctx, atom_val)
        };
        let val = unsafe {
            let js_val = qjs::JS_GetModuleExportEntry(self.m.ctx.ctx, ptr, self.cur);
            Value::from_js_value(self.m.ctx, js_val).unwrap()
        };
        self.cur += 1;
        Some((atom, val))
    }
}

/// Javascript module with certain exports and imports
#[derive(Debug, Clone)]
pub struct Module<'js> {
    ptr: *mut qjs::JSModuleDef,
    ctx: Ctx<'js>,
}

impl<'js> PartialEq<Module<'js>> for Module<'js> {
    fn eq(&self, other: &Module<'js>) -> bool {
        self.ptr == other.ptr
    }
}

impl<'js> Module<'js> {
    pub(crate) unsafe fn from_js_value(ctx: Ctx<'js>, js_val: qjs::JSValue) -> Self {
        debug_assert_eq!(js_val.tag, qjs::JS_TAG_MODULE as i64);
        let ptr = js_val.u.ptr;
        Module {
            ptr: ptr as *mut qjs::JSModuleDef,
            ctx,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn as_js_value(&self) -> qjs::JSValue {
        qjs::JSValue {
            u: qjs::JSValueUnion {
                ptr: self.ptr as *mut c_void,
            },
            tag: qjs::JS_TAG_MODULE as i64,
        }
    }

    /*pub fn new(ctx: Ctx<'js>, name: &str) -> Result<Self>{
        let name = CString::new(name)?;
        qjs::JS_NewCModule(ctx.ctx,name.as_ptr(),
    }*/

    #[cfg(feature = "exports")]
    pub fn export_list(&self) -> ExportList<'js> {
        ExportList {
            m: self.clone(),
            max: unsafe { qjs::JS_GetModuleExportEntriesCount(self.ptr) },
            cur: 0,
        }
    }

    #[doc(hidden)]
    #[cfg(feature = "exports")]
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
