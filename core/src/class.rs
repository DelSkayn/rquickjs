//! JavaScript classes defined from Rust.

use crate::{
    function::Params,
    qjs::{self},
    value::Constructor,
    Atom, Ctx, Error, FromJs, IntoJs, JsLifetime, Object, Result, Value,
};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::{hash::Hash, marker::PhantomData, mem, ops::Deref, ptr::NonNull};

mod cell;
mod trace;

pub(crate) mod ffi;

pub use cell::{
    Borrow, BorrowMut, JsCell, Mutability, OwnedBorrow, OwnedBorrowMut, Readable, Writable,
};
use ffi::{ClassCell, VTable};
pub use trace::{Trace, Tracer};
#[doc(hidden)]
pub mod impl_;

#[cfg(test)]
mod test;

/// The kind of a JavaScript class.
///
/// A class can't be both callable and exotic, so this enum encodes that constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    /// A regular class.
    Plain,
    /// A callable class (i.e. can be used as a function).
    Callable,
    /// An exotic class (i.e. has custom property access behavior).
    Exotic,
}

/// A JavaScript property descriptor returned from [`JsClass::exotic_get_own_property`].
pub struct PropertyDescriptor<'js> {
    /// The property value (for data descriptors).
    pub value: Value<'js>,
    /// The getter function (for accessor descriptors).
    pub getter: Value<'js>,
    /// The setter function (for accessor descriptors).
    pub setter: Value<'js>,
    /// Whether the property is configurable.
    pub configurable: bool,
    /// Whether the property is enumerable.
    pub enumerable: bool,
    /// Whether the property is writable (data descriptors only).
    pub writable: bool,
    /// Whether this is a getter/setter descriptor.
    pub is_getset: bool,
}

impl<'js> PropertyDescriptor<'js> {
    /// Create a simple value property descriptor.
    pub fn new_value(
        value: Value<'js>,
        configurable: bool,
        enumerable: bool,
        writable: bool,
    ) -> Self {
        let ctx = value.ctx().clone();
        PropertyDescriptor {
            value,
            getter: Value::new_undefined(ctx.clone()),
            setter: Value::new_undefined(ctx),
            configurable,
            enumerable,
            writable,
            is_getset: false,
        }
    }
}

/// A property name entry returned from [`JsClass::exotic_get_own_property_names`].
pub struct PropertyName<'js> {
    /// The atom identifying the property.
    pub atom: Atom<'js>,
    /// Whether this property is enumerable.
    pub is_enumerable: bool,
}

/// The trait which allows Rust types to be used from JavaScript.
pub trait JsClass<'js>: Trace<'js> + JsLifetime<'js> + Sized {
    /// The name the constructor has in JavaScript
    const NAME: &'static str;

    /// The kind of this class (plain, callable, or exotic).
    const KIND: ClassKind = ClassKind::Plain;

    /// Can the type be mutated while a JavaScript value.
    ///
    /// This should either be [`Readable`] or [`Writable`].
    type Mutable: Mutability;

    /// Returns the class prototype,
    fn prototype(ctx: &Ctx<'js>) -> Result<Option<Object<'js>>> {
        Object::new(ctx.clone()).map(Some)
    }

    /// Returns a predefined constructor for this specific class type if there is one.
    fn constructor(ctx: &Ctx<'js>) -> Result<Option<Constructor<'js>>>;

    /// The function which will be called if [`Self::KIND`] is [`ClassKind::Callable`] and an object with this
    /// class is called as if it is a function.
    fn call<'a>(this: &JsCell<'js, Self>, params: Params<'a, 'js>) -> Result<Value<'js>> {
        let _ = this;
        Ok(Value::new_undefined(params.ctx().clone()))
    }

    /// The function which will be called if a get property is performed on an object with this class
    fn exotic_get_property(
        this: &JsCell<'js, Self>,
        ctx: &Ctx<'js>,
        _atom: Atom<'js>,
        _receiver: Value<'js>,
    ) -> Result<Value<'js>> {
        let _ = this;
        Ok(Value::new_undefined(ctx.clone()))
    }

    /// The function which will be called if a set property is performed on an object with this class
    fn exotic_set_property(
        this: &JsCell<'js, Self>,
        _ctx: &Ctx<'js>,
        _atom: Atom<'js>,
        _receiver: Value<'js>,
        _value: Value<'js>,
    ) -> Result<bool> {
        let _ = this;
        Ok(false)
    }

    /// The function which will be called if a delete property is performed on an object with this class
    fn exotic_delete_property(
        this: &JsCell<'js, Self>,
        _ctx: &Ctx<'js>,
        _atom: Atom<'js>,
    ) -> Result<bool> {
        let _ = this;
        Ok(false)
    }

    /// The function which will be called if has property or similar is called on an object with this class
    fn exotic_has_property(
        this: &JsCell<'js, Self>,
        _ctx: &Ctx<'js>,
        _atom: Atom<'js>,
    ) -> Result<bool> {
        let _ = this;
        Ok(false)
    }

    /// Called to get the own property descriptor for a given property name.
    ///
    /// Return `Ok(Some(descriptor))` if the property exists, `Ok(None)` if it doesn't.
    fn exotic_get_own_property(
        this: &JsCell<'js, Self>,
        _ctx: &Ctx<'js>,
        _atom: Atom<'js>,
    ) -> Result<Option<PropertyDescriptor<'js>>> {
        let _ = this;
        Ok(None)
    }

    /// Called to enumerate the own property names of this object.
    ///
    /// Return a list of property names.
    fn exotic_get_own_property_names(
        this: &JsCell<'js, Self>,
        _ctx: &Ctx<'js>,
    ) -> Result<Vec<PropertyName<'js>>> {
        let _ = this;
        Ok(Vec::new())
    }
}

/// A object which is instance of a Rust class.
#[repr(transparent)]
pub struct Class<'js, C: JsClass<'js>>(pub(crate) Object<'js>, PhantomData<C>);

impl<'js, C: JsClass<'js>> Clone for Class<'js, C> {
    fn clone(&self) -> Self {
        Class(self.0.clone(), PhantomData)
    }
}

impl<'js, C: JsClass<'js>> PartialEq for Class<'js, C> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<'js, C: JsClass<'js>> Eq for Class<'js, C> {}

impl<'js, C: JsClass<'js>> Hash for Class<'js, C> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

unsafe impl<'js, C> JsLifetime<'js> for Class<'js, C>
where
    C: JsClass<'js> + JsLifetime<'js>,
    for<'to> C::Changed<'to>: JsClass<'to>,
{
    type Changed<'to> = Class<'to, C::Changed<'to>>;
}

impl<'js, C: JsClass<'js>> Deref for Class<'js, C> {
    type Target = Object<'js>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'js, C: JsClass<'js>> Class<'js, C> {
    /// Create a class from a Rust object.
    pub fn instance(ctx: Ctx<'js>, value: C) -> Result<Class<'js, C>> {
        let id = unsafe { class_id::<C>(&ctx)? };

        let prototype = Self::prototype(&ctx)?;

        let prototype = prototype.map(|x| x.as_js_value()).unwrap_or(qjs::JS_NULL);
        let val = unsafe {
            ctx.handle_exception(qjs::JS_NewObjectProtoClass(ctx.as_ptr(), prototype, id))?
        };

        let ptr = Box::into_raw(Box::new(ClassCell::new(value)));
        unsafe { qjs::JS_SetOpaque(val, ptr.cast()) };
        Ok(Self(
            unsafe { Object::from_js_value(ctx, val) },
            PhantomData,
        ))
    }

    /// Create a class from a Rust object with a given prototype.
    pub fn instance_proto(value: C, proto: Object<'js>) -> Result<Class<'js, C>> {
        let id = unsafe { class_id::<C>(proto.ctx())? };

        let val = unsafe {
            proto.ctx.handle_exception(qjs::JS_NewObjectProtoClass(
                proto.ctx().as_ptr(),
                proto.0.as_js_value(),
                id,
            ))?
        };
        let ptr = Box::into_raw(Box::new(ClassCell::new(value)));
        unsafe { qjs::JS_SetOpaque(val, ptr.cast()) };
        Ok(Self(
            unsafe { Object::from_js_value(proto.ctx.clone(), val) },
            PhantomData,
        ))
    }

    /// Returns the prototype for the class.
    ///
    /// Returns `None` if the class is not yet registered or if the class doesn't have a prototype.
    pub fn prototype(ctx: &Ctx<'js>) -> Result<Option<Object<'js>>> {
        unsafe { ctx.get_context_opaque().get_or_insert_prototype::<C>(ctx) }
    }

    /// Create a constructor for the current class using its definition.
    pub fn create_constructor(ctx: &Ctx<'js>) -> Result<Option<Constructor<'js>>> {
        C::constructor(ctx)
    }

    /// Defines the predefined constructor of this class, if there is one, onto the given object.
    pub fn define(object: &Object<'js>) -> Result<()> {
        if let Some(constructor) = Self::create_constructor(object.ctx())? {
            object.set(C::NAME, constructor)?;
        }
        Ok(())
    }

    /// Returns a reference to the underlying object contained in a cell.
    #[inline]
    pub(crate) fn get_class_cell<'a>(&'a self) -> &'a ClassCell<JsCell<'js, C>> {
        unsafe { self.get_class_ptr().as_ref() }
    }

    /// Returns a reference to the underlying object contained in a cell.
    #[inline]
    pub fn get_cell<'a>(&'a self) -> &'a JsCell<'js, C> {
        &self.get_class_cell().data
    }

    /// Borrow the Rust class type.
    ///
    /// JavaScript classes behave similar to [`Rc`](std::rc::Rc) in Rust, you can essentially think
    /// of a class object as a `Rc<RefCell<C>>` and with similar borrowing functionality.
    ///
    /// # Panic
    /// This function panics if the class is already borrowed mutably.
    #[inline]
    pub fn borrow<'a>(&'a self) -> Borrow<'a, 'js, C> {
        self.get_cell().borrow()
    }

    /// Borrow the Rust class type mutably.
    ///
    /// JavaScript classes behave similar to [`Rc`](std::rc::Rc) in Rust, you can essentially think
    /// of a class object as a `Rc<RefCell<C>>` and with similar borrowing functionality.
    ///
    /// # Panic
    /// This function panics if the class is already borrowed mutably or immutably, or the Class
    /// can't be borrowed mutably.
    #[inline]
    pub fn borrow_mut<'a>(&'a self) -> BorrowMut<'a, 'js, C> {
        self.get_cell().borrow_mut()
    }

    /// Try to borrow the Rust class type.
    ///
    /// JavaScript classes behave similar to [`Rc`](std::rc::Rc) in Rust, you can essentially think
    /// of a class object as a `Rc<RefCell<C>>` and with similar borrowing functionality.
    ///
    /// This returns an error when the class is already borrowed mutably.
    #[inline]
    pub fn try_borrow<'a>(&'a self) -> Result<Borrow<'a, 'js, C>> {
        self.get_cell().try_borrow().map_err(Error::ClassBorrow)
    }

    /// Try to borrow the Rust class type mutably.
    ///
    /// JavaScript classes behave similar to [`Rc`](std::rc::Rc) in Rust, you can essentially think
    /// of a class object as a `Rc<RefCell<C>>` and with similar borrowing functionality.
    ///
    /// This returns an error when the class is already borrowed mutably, immutably or the class
    /// can't be borrowed mutably.
    #[inline]
    pub fn try_borrow_mut<'a>(&'a self) -> Result<BorrowMut<'a, 'js, C>> {
        self.get_cell().try_borrow_mut().map_err(Error::ClassBorrow)
    }

    /// returns a pointer to the class object.
    #[inline]
    pub(crate) fn get_class_ptr(&self) -> NonNull<ClassCell<JsCell<'js, C>>> {
        let id = unsafe { class_id::<C>(&self.ctx).expect("invalid class") };

        let ptr = unsafe { qjs::JS_GetOpaque2(self.0.ctx.as_ptr(), self.0 .0.as_js_value(), id) };

        NonNull::new(ptr.cast()).expect("invalid class object, object didn't have opaque value")
    }

    /// Turns the class back into a generic object.
    #[inline]
    pub fn into_inner(self) -> Object<'js> {
        self.0
    }

    /// Turns the class back into a generic object.
    #[inline]
    pub fn as_inner(&self) -> &Object<'js> {
        &self.0
    }

    /// Convert from value.
    #[inline]
    pub fn from_value(value: &Value<'js>) -> Result<Self> {
        if let Some(cls) = value.as_object().and_then(Self::from_object) {
            return Ok(cls);
        }
        Err(Error::FromJs {
            from: value.type_name(),
            to: C::NAME,
            message: None,
        })
    }

    /// Turn the class into a value.
    #[inline]
    pub fn into_value(self) -> Value<'js> {
        self.0.into_value()
    }

    /// Converts a generic object into a class if the object is of the right class.
    #[inline]
    pub fn from_object(object: &Object<'js>) -> Option<Self> {
        object.into_class().ok()
    }
}

impl<'js> Object<'js> {
    /// Returns if the object is of a certain Rust class.
    pub fn instance_of<C: JsClass<'js>>(&self) -> bool {
        let Ok(id) = (unsafe { class_id::<C>(&self.ctx) }) else {
            return false;
        };

        // This checks if the class is of the right class id.
        let Some(x) = NonNull::new(unsafe {
            qjs::JS_GetOpaque2(self.0.ctx.as_ptr(), self.0.as_js_value(), id)
        }) else {
            // `JS_GetOpaque2` throws a `TypeError` internally whenever the class id
            // doesn't match, leaving it pending on the runtime even though a
            // negative result here is an ordinary, expected outcome. Clear it so a
            // failed `instance_of` can't be mistaken for a real exception by code
            // that later checks `Ctx::has_exception`/`Ctx::catch` for something
            // unrelated (e.g. a pending `Promise`).
            self.ctx.catch();
            return false;
        };

        let v_table = unsafe { x.cast::<ClassCell<()>>().as_ref().v_table };

        // If the pointer is equal it must be of the right type, as the inclusion of a call to
        // generate a TypeId means that each type must have a unique v table.
        // however if it is not equal then it can still be the right type if the v_table is
        // duplicated, which is possible when compilation with multiple code-gen units.
        //
        // Doing check avoids a lookup and an dynamic function call in some cases.
        if core::ptr::eq(v_table, VTable::get::<C>()) {
            return true;
        }

        v_table.is_of_class::<C>()
    }

    /// Turn the object into the class if it is an instance of that class.
    pub fn into_class<C: JsClass<'js>>(&self) -> core::result::Result<Class<'js, C>, &Self> {
        if self.instance_of::<C>() {
            Ok(Class(self.clone(), PhantomData))
        } else {
            Err(self)
        }
    }

    /// Turn the object into the class if it is an instance of that class.
    pub fn as_class<C: JsClass<'js>>(&self) -> Option<&Class<'js, C>> {
        if self.instance_of::<C>() {
            // SAFETY:
            // Safe because class is a transparent wrapper
            unsafe { Some(mem::transmute::<&Object<'js>, &Class<'js, C>>(self)) }
        } else {
            None
        }
    }
}

impl<'js, C: JsClass<'js>> FromJs<'js> for Class<'js, C> {
    fn from_js(_ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Self::from_value(&value)
    }
}

impl<'js, C: JsClass<'js>> IntoJs<'js> for Class<'js, C> {
    fn into_js(self, _ctx: &Ctx<'js>) -> Result<Value<'js>> {
        Ok(self.0 .0)
    }
}

unsafe fn class_id<'js, C: JsClass<'js>>(ctx: &Ctx<'js>) -> Result<qjs::JSClassID> {
    match C::KIND {
        ClassKind::Plain => Ok(ctx.get_runtime_opaque().get_class_id()),
        ClassKind::Callable => Ok(ctx.get_runtime_opaque().get_callable_id()),
        ClassKind::Exotic => Ok(ctx.get_runtime_opaque().get_exotic_id()),
    }
}
