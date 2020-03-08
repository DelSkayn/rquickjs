use super::ToAtom;
use crate::{value::*, Ctx};
use std::string::String as StdString;

impl<'js> ToAtom<'js> for Atom<'js> {
    fn to_atom(self, _: Ctx<'js>) -> Atom<'js> {
        self
    }
}

impl<'js> ToAtom<'js> for Value<'js> {
    fn to_atom(self, ctx: Ctx<'js>) -> Atom<'js> {
        Atom::from_value(ctx, &self)
    }
}

impl<'js> ToAtom<'js> for &str {
    fn to_atom(self, ctx: Ctx<'js>) -> Atom<'js> {
        Atom::from_str(ctx, self)
    }
}

impl<'js> ToAtom<'js> for StdString {
    fn to_atom(self, ctx: Ctx<'js>) -> Atom<'js> {
        Atom::from_str(ctx, &self)
    }
}

impl<'js> ToAtom<'js> for u32 {
    fn to_atom(self, ctx: Ctx<'js>) -> Atom<'js> {
        Atom::from_u32(ctx, self)
    }
}

macro_rules! impl_for_to_js(
    ($ty:ident, $Var:ident) => {
        impl<'js> ToAtom<'js> for $ty{
            fn to_atom(self, ctx: Ctx<'js>) -> Atom<'js>{
                Atom::from_value(ctx,&Value::$Var(self))
            }
        }
    }
);

macro_rules! impl_for_to_js_lt(
    ($ty:ident, $Var:ident) => {
        impl<'js> ToAtom<'js> for $ty<'js>{
            fn to_atom(self, ctx: Ctx<'js>) -> Atom<'js>{
                Atom::from_value(ctx,&Value::$Var(self))
            }
        }
    }
);

impl_for_to_js_lt!(String, String);
impl_for_to_js_lt!(Object, Object);
impl_for_to_js_lt!(Array, Array);
impl_for_to_js_lt!(Function, Function);
impl_for_to_js!(i32, Int);
impl_for_to_js!(bool, Bool);
