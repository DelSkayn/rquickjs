use crate::{
    atom::PredefinedAtom, qjs, Atom, Ctx, FromAtom, IntoAtom, Result, StdString, String, Value,
};

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

impl<'js> IntoAtom<'js> for PredefinedAtom {
    fn into_atom(self, ctx: Ctx<'js>) -> Result<Atom<'js>> {
        Ok(unsafe { Atom::from_atom_val_dup(ctx, self as qjs::JSAtom) })
    }
}

impl<'js> IntoAtom<'js> for Atom<'js> {
    fn into_atom(self, _: Ctx<'js>) -> Result<Atom<'js>> {
        Ok(self)
    }
}

impl<'js> IntoAtom<'js> for Value<'js> {
    fn into_atom(self, ctx: Ctx<'js>) -> Result<Atom<'js>> {
        Atom::from_value(ctx, &self)
    }
}

impl<'js> IntoAtom<'js> for &str {
    fn into_atom(self, ctx: Ctx<'js>) -> Result<Atom<'js>> {
        Atom::from_str(ctx, self)
    }
}

impl<'js> IntoAtom<'js> for StdString {
    fn into_atom(self, ctx: Ctx<'js>) -> Result<Atom<'js>> {
        Atom::from_str(ctx, &self)
    }
}

impl<'js> IntoAtom<'js> for &StdString {
    fn into_atom(self, ctx: Ctx<'js>) -> Result<Atom<'js>> {
        Atom::from_str(ctx, self)
    }
}

macro_rules! into_atom_impls {
	  ($($from:ident: $($type:ident)*,)*) => {
		    $(
            $(
                impl<'js> IntoAtom<'js> for $type {
                    fn into_atom(self, ctx: Ctx<'js>) -> Result<Atom<'js>> {
                        Atom::$from(ctx, self as _)
                    }
                }
            )*
        )*
	  };
}

into_atom_impls! {
    from_bool: bool,
    from_u32: u8 u16 u32,
    from_i32: i8 i16 i32,
    from_f64: f32 f64,
}
