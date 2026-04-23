//! Helper classes and functions for use inside the macros.

use crate::{class::JsClass, value::Constructor, Ctx, Object, Result};
use core::marker::PhantomData;

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

/// Specialization isn't stabilized yet so in the macro we can't normally have a default
/// implementation for class prototypes if it doesn't have an associated impl item.
///
/// We would need this default implementation because it is not possible to know in class macro if
/// the method macro is triggered for a impl body of the same class.
///
/// We can get around this by using a bit of a hack called autoref-specialization.
///
/// [`MethodImplementor`] is implemented for a borrowed value generic over any `T`;
/// This means you will always be able to call `(&&MethodImpl<T>).implement(proto)` because it is
/// implemented for all `&MethodImpl<T>`.
///
/// However it the trait is also implemented for `MethodImpl<Foo>` for some specific class `Foo` the
/// compiler will automatically deref the first reference and call the method for the type
/// `MethodImpl<Foo>` instead of the general on.
///
/// This allows us to provide a default implementation if no implementation of [`MethodImplementor`] is
/// present for `T`.
///
/// Originally described by dtolnay
impl<T> MethodImplementor<T> for &MethodImpl<T> {}

impl<'js, T> ConstructorCreator<'js, T> for &ConstructorCreate<T> {}

/// A helper struct to implement [`FromJs`](crate::FromJs) for types which implement [`Clone`].
pub struct CloneWrapper<'a, T>(pub &'a T);
/// A helper trait to implement [`FromJs`](crate::FromJs) for types which implement [`Clone`].
pub trait CloneTrait<T> {
    fn wrap_clone(&self) -> T;
}

impl<'a, T: Clone> CloneTrait<T> for CloneWrapper<'a, T> {
    fn wrap_clone(&self) -> T {
        self.0.clone()
    }
}

/// Compile-time check used by the `#[rquickjs::class]` macro to reject
/// fields with `#[qjs(get)]`/`#[qjs(set)]` that are themselves a [`JsClass`]
/// type.
///
/// Such fields do not round-trip with reference semantics: the generated
/// getter clones the value and wraps the clone in a fresh class instance, so
/// nested mutations (e.g. `obj.b.c = x`) are discarded. Wrap the field in a
/// [`Class<'js, T>`](crate::class::Class) instead to share the underlying cell.
///
/// Uses autoref-specialization: the inherent `check` (deprecated) is picked
/// when `T: JsClass`, otherwise the trait fallback below is used.
#[derive(Default)]
pub struct JsClassFieldCheck<T>(PhantomData<T>);

impl<T> JsClassFieldCheck<T> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

/// Marker trait with no implementations. Used purely for its
/// [`on_unimplemented`](https://doc.rust-lang.org/reference/attributes/diagnostics.html#the-diagnosticon_unimplemented-attribute)
/// diagnostic: when `JsClassFieldCheck::<T>::check()` is selected via
/// autoref-specialization for a `T: JsClass`, its `where T: NotAJsClassField`
/// bound fails and rustc emits this custom message.
#[diagnostic::on_unimplemented(
    message = "using a `JsClass` type directly as a class field is not supported",
    label = "`{Self}` implements `JsClass` \u{2014} wrap the field in `Class<'js, T>` instead",
    note = "nested mutations are lost because the generated getter clones the value"
)]
pub trait NotAJsClassField {}

impl<'js, T: JsClass<'js>> JsClassFieldCheck<T> {
    pub fn check(self)
    where
        T: NotAJsClassField,
    {
    }
}

pub trait JsClassFieldCheckFallback {
    fn check(self);
}

impl<T> JsClassFieldCheckFallback for &JsClassFieldCheck<T> {
    fn check(self) {}
}
