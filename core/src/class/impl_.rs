use std::marker::PhantomData;

use crate::{Ctx, Object, Result};

/// Specializer for implemeting methods for classes
pub struct MethodImplementor<T>(PhantomData<T>);

impl<T> MethodImplementor<T> {
    pub const fn new() -> Self {
        MethodImplementor(PhantomData)
    }
}

pub trait ImplMethod<T> {
    fn init_proto<'js>(&self, ctx: Ctx<'js>, proto: &Object<'js>) -> Result<()>;
}

impl<T> ImplMethod<T> for &MethodImplementor<T> {
    fn init_proto<'js>(&self, _ctx: Ctx<'js>, _proto: &Object<'js>) -> Result<()> {
        Ok(())
    }
}
