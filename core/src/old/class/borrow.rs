use std::{fmt, ops::Deref};

use crate::{class::ClassDef, Class, Ctx, FromJs, IntoJs, Result, Value};

pub struct Ref<'js, C>(Class<'js, C>);

impl<'js, C: ClassDef + fmt::Debug> fmt::Debug for Ref<'js, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Ref").field(self.deref()).finish()
    }
}

impl<'js, C: ClassDef> Deref for Ref<'js, C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        self.get_ref()
    }
}

impl<'js, C: ClassDef> Ref<'js, C> {
    pub fn new(class: Class<'js, C>) -> Self {
        Ref(class)
    }

    pub fn into_inner(self) -> Class<'js, C> {
        self.0
    }

    fn get_ref(&self) -> &C {
        unsafe {
            let ptr = self.0.class_ptr();
            &*ptr
        }
    }
}

impl<'js, C: ClassDef> FromJs<'js> for Ref<'js, C> {
    fn from_js(ctx: Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Ok(Ref(Class::<C>::from_js(ctx, value)?))
    }
}

impl<'js, C: ClassDef> IntoJs<'js> for Ref<'js, C> {
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        self.0.into_js(ctx)
    }
}
