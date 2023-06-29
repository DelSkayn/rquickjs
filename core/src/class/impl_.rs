use std::marker::PhantomData;

use crate::{Object, Result};

pub trait MethodImplementor {
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

impl<T> MethodImplementor for &MethodImpl<T> {}
