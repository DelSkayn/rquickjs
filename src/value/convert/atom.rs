use super::{FromAtom, IntoAtom};
use crate::{value::*, Ctx, Result};
use std::string::String as StdString;

impl<'js> FromAtom<'js> for Atom<'js> {
    fn from_atom(atom: Atom<'js>) -> Result<Self> {
        Ok(atom)
    }
}

impl<'js> FromAtom<'js> for Value<'js> {
    fn from_atom(atom: Atom<'js>) -> Result<Self> {
        atom.to_value()
    }
}

impl<'js> FromAtom<'js> for String<'js> {
    fn from_atom(atom: Atom<'js>) -> Result<Self> {
        atom.to_js_string()
    }
}

impl<'js> FromAtom<'js> for StdString {
    fn from_atom(atom: Atom<'js>) -> Result<Self> {
        atom.to_string()
    }
}

impl<'js> IntoAtom<'js> for Atom<'js> {
    fn to_atom(self, _: Ctx<'js>) -> Atom<'js> {
        self
    }
}

impl<'js> IntoAtom<'js> for Value<'js> {
    fn to_atom(self, ctx: Ctx<'js>) -> Atom<'js> {
        Atom::from_value(ctx, &self)
    }
}

impl<'js> IntoAtom<'js> for &str {
    fn to_atom(self, ctx: Ctx<'js>) -> Atom<'js> {
        Atom::from_str(ctx, self)
    }
}

impl<'js> IntoAtom<'js> for StdString {
    fn to_atom(self, ctx: Ctx<'js>) -> Atom<'js> {
        Atom::from_str(ctx, &self)
    }
}

impl<'js> IntoAtom<'js> for u32 {
    fn to_atom(self, ctx: Ctx<'js>) -> Atom<'js> {
        Atom::from_u32(ctx, self)
    }
}

macro_rules! impl_for_to_js(
    ($ty:ty, $Var:ident) => {
        impl<'js> IntoAtom<'js> for $ty {
            fn to_atom(self, ctx: Ctx<'js>) -> Atom<'js> {
                Atom::from_value(ctx, &Value::$Var(self))
            }
        }
    }
);

impl_for_to_js!(String<'js>, String);
impl_for_to_js!(Object<'js>, Object);
impl_for_to_js!(Array<'js>, Array);
impl_for_to_js!(Function<'js>, Function);
impl_for_to_js!(i32, Int);
impl_for_to_js!(bool, Bool);
