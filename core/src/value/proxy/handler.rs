use alloc::string::String as StdString;

use crate::{atom::PredefinedAtom, Ctx, Error, FromJs, Function, IntoJs, Object, Result, Value};

/// Helper type for the proxy target
#[derive(Debug, PartialEq, Clone, Hash, Eq)]
pub struct ProxyTarget<'js>(pub Object<'js>);

impl<'js> FromJs<'js> for ProxyTarget<'js> {
    fn from_js(_: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Ok(Self(Object::from_value(value)?))
    }
}

/// Helper type for the proxy property
#[derive(Debug, PartialEq, Clone, Hash, Eq)]
pub struct ProxyProperty<'js>(pub Value<'js>);

impl<'js> ProxyProperty<'js> {
    pub fn is_symbol(&self) -> bool {
        self.0.is_symbol()
    }

    pub fn is_string(&self) -> bool {
        self.0.is_string()
    }

    pub fn to_string(&self) -> Result<StdString> {
        if let Some(string) = self.0.as_string() {
            string.to_string()
        } else if let Some(symbol) = self.0.as_symbol() {
            symbol.as_atom().to_string()
        } else {
            Err(Error::Unknown)
        }
    }
}

impl<'js> FromJs<'js> for ProxyProperty<'js> {
    fn from_js(_: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Ok(Self(value))
    }
}

/// Helper type for the proxy handler
#[derive(Debug, PartialEq, Clone, Hash, Eq)]
pub struct ProxyReceiver<'js>(pub Value<'js>);

impl<'js> FromJs<'js> for ProxyReceiver<'js> {
    fn from_js(_: &Ctx<'js>, value: Value<'js>) -> Result<Self> {
        Ok(Self(value))
    }
}

/// Rust representation of a JavaScript proxy handler.
#[derive(Debug, PartialEq, Clone, Hash, Eq)]
#[repr(transparent)]
pub struct ProxyHandler<'js>(pub(crate) Object<'js>);

impl<'js> ProxyHandler<'js> {
    /// Create a new empty proxy handler
    pub fn new(ctx: Ctx<'js>) -> Result<Self> {
        Ok(Self(Object::new(ctx)?))
    }

    /// Create a new proxy handler from a existing object
    pub fn from_object(object: Object<'js>) -> Result<Self> {
        Ok(Self(object))
    }

    /// Set the getter function for the proxy handler
    pub fn set_getter<F, V>(&self, get: F) -> Result<()>
    where
        F: Fn(ProxyTarget<'js>, ProxyProperty<'js>, ProxyReceiver<'js>) -> Result<V> + 'js,
        V: IntoJs<'js> + 'js,
    {
        self.0.set(
            PredefinedAtom::Getter,
            Function::new(self.0.ctx().clone(), get)?,
        )?;
        Ok(())
    }

    /// Set the getter function for the proxy handler
    pub fn with_getter<F, V>(self, get: F) -> Result<Self>
    where
        F: Fn(ProxyTarget<'js>, ProxyProperty<'js>, ProxyReceiver<'js>) -> Result<V> + 'js,
        V: IntoJs<'js> + 'js,
    {
        self.set_getter(get)?;
        Ok(self)
    }

    /// Set the setter function for the proxy handler
    pub fn set_setter<F>(&self, set: F) -> Result<()>
    where
        F: Fn(ProxyTarget<'js>, ProxyProperty<'js>, Value<'js>, ProxyReceiver<'js>) -> Result<bool>
            + 'js,
    {
        self.0.set(
            PredefinedAtom::Setter,
            Function::new(self.0.ctx().clone(), set)?,
        )?;
        Ok(())
    }

    /// Set the setter function for the proxy handler
    pub fn with_setter<F>(self, set: F) -> Result<Self>
    where
        F: Fn(ProxyTarget<'js>, ProxyProperty<'js>, Value<'js>, ProxyReceiver<'js>) -> Result<bool>
            + 'js,
    {
        self.set_setter(set)?;
        Ok(self)
    }

    /// Set the has function for the proxy handler
    pub fn set_has<F>(&self, has: F) -> Result<()>
    where
        F: Fn(ProxyTarget<'js>, ProxyProperty<'js>) -> Result<bool> + 'js,
    {
        self.0.set(
            PredefinedAtom::Has,
            Function::new(self.0.ctx().clone(), has)?,
        )?;
        Ok(())
    }

    /// Set the has function for the proxy handler
    pub fn with_has<F>(self, has: F) -> Result<Self>
    where
        F: Fn(ProxyTarget<'js>, ProxyProperty<'js>) -> Result<bool> + 'js,
    {
        self.set_has(has)?;
        Ok(self)
    }

    /// Set the delete function for the proxy handler
    pub fn set_delete<F>(&self, delete: F) -> Result<()>
    where
        F: Fn(ProxyTarget<'js>, ProxyProperty<'js>) -> Result<bool> + 'js,
    {
        self.0.set(
            PredefinedAtom::DeleteProperty,
            Function::new(self.0.ctx().clone(), delete)?,
        )?;
        Ok(())
    }

    /// Set the delete function for the proxy handler
    pub fn with_delete<F>(self, delete: F) -> Result<Self>
    where
        F: Fn(ProxyTarget<'js>, ProxyProperty<'js>) -> Result<bool> + 'js,
    {
        self.set_delete(delete)?;
        Ok(self)
    }
}

impl<'js> IntoJs<'js> for ProxyHandler<'js> {
    fn into_js(self, ctx: &Ctx<'js>) -> Result<Value<'js>> {
        self.0.into_js(ctx)
    }
}
