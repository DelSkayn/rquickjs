//! Helper classes and functions for use inside the macros.

use std::marker::PhantomData;

use crate::{value::Constructor, Ctx, Object, Result};

/// Trait used for borrow specialization for implementing methods without access to the class.
pub trait MethodImplementor<T>: Sized {
    fn implement<'js>(&self, _proto: &Object<'js>) -> Result<()> {
        Ok(())
    }
}

/// Trait used for borrow specialization for creating constructors without access to the class.
pub trait ConstructorCreator<'js, T>: Sized {
    fn create_constructor(&self, _ctx: &Ctx<'js>) -> Result<Option<Constructor<'js>>> {
        Ok(None)
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

/// A helper type for borrow specialization
#[derive(Default)]
pub struct ConstructorCreate<T>(PhantomData<T>);

impl<T> ConstructorCreate<T> {
    pub fn new() -> Self {
        ConstructorCreate(PhantomData)
    }
}

/// Specialization isn't stablized yet so in the macro we can't normally have a default
/// implementation for class prototypes if it doesn't have an associated impl item.
///
/// We would need this default implementation because it is not possible to know in class macro if
/// the method macro is triggered for a impl body of the same class.
///
/// We can get around this by using a bit of a heck called autoref-specialization.
///
/// MethodImplementor is implemented for a borrowed value generic over any T;
/// This means you will always be able to call `(&&MethodImpl<T>).implement(proto)` because it is
/// implemented for all &MethodImpl<T>.
///
/// However it the trait is also implemented for MethodImpl<Foo> for some specific class Foo the
/// compiler will automaticall deref the first reference and call the method for the type
/// MethodImpl<Foo> instead of the general on.
///
/// This allows us to provide a default implementation if no implementation of MethodImplementor is
/// present for T.
///
/// Originally described by dtolnay
impl<T> MethodImplementor<T> for &MethodImpl<T> {}

impl<'js, T> ConstructorCreator<'js, T> for &ConstructorCreate<T> {}

/// A helper struct to implement FromJs for types which implement clone.
pub struct CloneWrapper<'a, T>(pub &'a T);
/// A helper trait to implement FromJs for types which implement clone.
pub trait CloneTrait<T> {
    fn wrap_clone(&self) -> T;
}

impl<'a, T: Clone> CloneTrait<T> for CloneWrapper<'a, T> {
    fn wrap_clone(&self) -> T {
        self.0.clone()
    }
}
