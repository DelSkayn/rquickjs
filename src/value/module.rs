use crate::Ctx;
#[cfg(feature = "exports")]
use crate::{Atom, Value};
use rquickjs_sys as qjs;

/// An iterator over the items exported out a module
///
/// # Features
/// This struct is only availble if the `exports` feature is enabled.
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
        debug_assert_eq!(qjs::JS_VALUE_GET_NORM_TAG(js_val), qjs::JS_TAG_MODULE);
        let ptr = qjs::JS_VALUE_GET_PTR(js_val);
        Module {
            ptr: ptr as *mut qjs::JSModuleDef,
            ctx,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn as_js_value(&self) -> qjs::JSValue {
        qjs::JS_MKPTR(qjs::JS_TAG_MODULE, self.ptr as *mut _)
    }

    /// Returns the name of the module as a atom
    pub fn name(&self) -> Atom<'js> {
        unsafe { Atom::from_atom_val(self.ctx, qjs::JS_GetModuleName(self.ctx.ctx, self.ptr)) }
    }

    /// Returns a iterator over the items the module export.
    ///
    /// # Features
    /// This function is only availble if the `exports` feature is enabled.
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::*;

    #[test]
    fn from_javascript() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let val: Module = ctx
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
            assert_eq!(val.name().to_string().unwrap(), "Test".to_string());
            let mut iter = val.export_list();
            assert_eq!(iter.next().unwrap().0.to_string().unwrap(), "a".to_string());
            assert_eq!(
                iter.next().unwrap().0.to_string().unwrap(),
                "foo".to_string()
            );
            assert_eq!(
                iter.next().unwrap().0.to_string().unwrap(),
                "Baz".to_string()
            );
        });
    }
}
