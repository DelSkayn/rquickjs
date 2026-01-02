use crate::class::JsClass;

/// Trait for classes that have a parent class.
pub trait HasParent<'js>
where
    Self: JsClass<'js>,
{
    /// Since the JSCell has different memory layout for different mutabilities,
    /// the parent class must have the same mutability as the child class.
    type Parent: JsClass<'js, Mutable = <Self as JsClass<'js>>::Mutable>;

    fn as_parent(&self) -> &Self::Parent;
}
