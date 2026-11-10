use core::{any::TypeId, cell::UnsafeCell, ptr::NonNull};

use hashbrown::{hash_map::Entry, HashMap};

use crate::{
    class::{ffi::VTable, JsClass},
    qjs, Ctx, Error, Object,
};

enum Prototype<'js> {
    None,
    /// Value inserted to signify that we are currently building the prototype.
    /// If it is reached while building a prototype, than we have a recursive definition.
    Pending,
    Some(Object<'js>),
}

pub(crate) struct ContextOpaque<'js> {
    prototypes: UnsafeCell<HashMap<TypeId, Prototype<'js>>>,
}

impl<'js> ContextOpaque<'js> {
    pub fn new() -> Self {
        ContextOpaque {
            prototypes: UnsafeCell::new(HashMap::new()),
        }
    }

    pub fn from_context_ptr<'a>(ptr: NonNull<qjs::JSContext>) -> &'a Self {
        unsafe { &*qjs::JS_GetContextOpaque(ptr.as_ptr()).cast::<Self>() }
    }

    pub fn get_or_insert_prototype<C: JsClass<'js>>(
        &self,
        ctx: &Ctx<'js>,
    ) -> Result<Option<Object<'js>>, Error> {
        unsafe {
            let vtable = VTable::get::<C>();
            let id = vtable.id();

            // Insert in two steps to that we don't hold a mutable reference across
            // recursive calls to the function.
            {
                match (*self.prototypes.get()).entry(id) {
                    Entry::Occupied(x) => match x.get() {
                        Prototype::None => return Ok(None),
                        Prototype::Pending => return Err(Error::RecursivePrototype),
                        Prototype::Some(object) => return Ok(Some(object.clone())),
                    },
                    Entry::Vacant(x) => {
                        x.insert(Prototype::Pending);
                    }
                }
            }

            match C::prototype(ctx) {
                Ok(Some(proto)) => {
                    (*self.prototypes.get()).insert(id, Prototype::Some(proto.clone()));
                    Ok(Some(proto))
                }
                Ok(None) => {
                    (*self.prototypes.get()).insert(id, Prototype::None);
                    Ok(None)
                }
                Err(e) => {
                    (*self.prototypes.get()).remove(&id);
                    Err(e)
                }
            }
        }
    }
}
