use std::marker::PhantomData;

use crate::{Object, Result};

/// Trait used for borrow specialization for implementing methods without access to the class.
pub trait MethodImplementor<T> {
    fn implement<'js>(&self, _proto: &Object<'js>) -> Result<()> {
        Ok(())
    }
}

/// A helper type for borrow specialization
#[derive(Default)]
pub struct MethodImpl<T>(PhantomData<T>);

impl<T> MethodImpl<T> {
    pub fn new() -> Self {
        MethodImpl(PhantomData)
    }
}

/// MethodImplementor is implemented for a borrowed value generic over any T;
///
/// However if someone decides to implement it for a non borrowed value for a specific T.
/// Calling `MethodImplementor::implement(&MethodImpl<T>,proto)` will result in specialization in
/// the for of rust using the non borrowed implementation over the borrowed.
///
/// Originally described by .. TODO: Lookup
impl<T> MethodImplementor<T> for &MethodImpl<T> {}
