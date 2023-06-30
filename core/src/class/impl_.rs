use std::marker::PhantomData;

use crate::{Object, Result};

pub trait MethodImplementor<T> {
    fn implement<'js>(&self, _proto: &Object<'js>) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct MethodImpl<T>(PhantomData<T>);

impl<T> MethodImpl<T> {
    pub fn new() -> Self {
        MethodImpl(PhantomData)
    }
}

impl<T> MethodImplementor<T> for &MethodImpl<T> {}
