//!  QuickJS atom functionality.

use crate::{qjs, Ctx, Error, Result, String, Value};
use alloc::string::String as StdString;
use core::hash::Hash;

mod predefined;
pub use predefined::PredefinedAtom;

/// A QuickJS Atom.
///
/// In QuickJS atoms are similar to interned string but with additional uses.
/// A symbol for instance is just an atom.
///
/// # Representation
///
/// Atoms in QuickJS are handled differently depending on what type of index the represent.
/// When the atom represents a number like index, like `object[1]` the atom is just
/// a normal number.
/// However when the atom represents a string link index like `object["foo"]` or `object.foo`
/// the atom represents a value in a hashmap.
#[derive(Debug)]
pub struct Atom<'js> {
    pub(crate) atom: qjs::JSAtom,
    pub(crate) ctx: Ctx<'js>,
}

impl<'js> PartialEq for Atom<'js> {
    fn eq(&self, other: &Self) -> bool {
        self.atom == other.atom
    }
}
impl<'js> Eq for Atom<'js> {}

impl<'js> Hash for Atom<'js> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_u32(self.atom)
    }
}

impl<'js> Atom<'js> {
    /// Create an atom from a JavaScript value.
    pub fn from_value(ctx: Ctx<'js>, val: &Value<'js>) -> Result<Atom<'js>> {
        let atom = unsafe { qjs::JS_ValueToAtom(ctx.as_ptr(), val.as_js_value()) };
        if atom == qjs::JS_ATOM_NULL {
            // A value can be anything, including an object which might contain a callback so check
            // for panics.
            return Err(ctx.raise_exception());
        }
        Ok(Atom { atom, ctx })
    }

    /// Create an atom from a `u32`
    pub fn from_u32(ctx: Ctx<'js>, val: u32) -> Result<Atom<'js>> {
        let atom = unsafe { qjs::JS_NewAtomUInt32(ctx.as_ptr(), val) };
        if atom == qjs::JS_ATOM_NULL {
            // Should never invoke a callback so no panics
            return Err(Error::Exception);
        }
        Ok(Atom { atom, ctx })
    }

    /// Create an atom from an `i32` via value
    pub fn from_i32(ctx: Ctx<'js>, val: i32) -> Result<Atom<'js>> {
        let atom =
            unsafe { qjs::JS_ValueToAtom(ctx.as_ptr(), qjs::JS_MKVAL(qjs::JS_TAG_INT, val)) };
        if atom == qjs::JS_ATOM_NULL {
            // Should never invoke a callback so no panics
            return Err(Error::Exception);
        }
        Ok(Atom { atom, ctx })
    }

    /// Create an atom from a `bool` via value
    pub fn from_bool(ctx: Ctx<'js>, val: bool) -> Result<Atom<'js>> {
        let val = if val { qjs::JS_TRUE } else { qjs::JS_FALSE };
        let atom = unsafe { qjs::JS_ValueToAtom(ctx.as_ptr(), val) };
        if atom == qjs::JS_ATOM_NULL {
            // Should never invoke a callback so no panics
            return Err(Error::Exception);
        }
        Ok(Atom { atom, ctx })
    }

    /// Create an atom from a `f64` via value
    pub fn from_f64(ctx: Ctx<'js>, val: f64) -> Result<Atom<'js>> {
        let atom = unsafe { qjs::JS_ValueToAtom(ctx.as_ptr(), qjs::JS_NewFloat64(val)) };
        if atom == qjs::JS_ATOM_NULL {
            // Should never invoke a callback so no panics
            return Err(Error::Exception);
        }
        Ok(Atom { atom, ctx })
    }

    /// Create an atom from a Rust string
    pub fn from_str(ctx: Ctx<'js>, name: &str) -> Result<Atom<'js>> {
        unsafe {
            let ptr = name.as_ptr() as *const core::ffi::c_char;
            let atom = qjs::JS_NewAtomLen(ctx.as_ptr(), ptr, name.len() as _);
            if atom == qjs::JS_ATOM_NULL {
                // Should never invoke a callback so no panics
                return Err(Error::Exception);
            }
            Ok(Atom { atom, ctx })
        }
    }

    /// Create an atom from a predefined atom.
    pub fn from_predefined(ctx: Ctx<'js>, predefined: PredefinedAtom) -> Atom<'js> {
        unsafe { Atom::from_atom_val(ctx, predefined as qjs::JSAtom) }
    }

    /// Convert the atom to a Rust string.
    pub fn to_string(&self) -> Result<StdString> {
        self.to_js_string()?.to_string()
    }

    /// Convert the atom to a Rust string, replacing invalid UTF-8 sequences
    /// with the Unicode replacement character (`U+FFFD`).
    pub fn to_string_lossy(&self) -> Result<StdString> {
        self.to_js_string()?.to_string_lossy()
    }

    /// Convert the atom to a JavaScript string.
    pub fn to_js_string(&self) -> Result<String<'js>> {
        unsafe {
            let val = qjs::JS_AtomToString(self.ctx.as_ptr(), self.atom);
            let val = self.ctx.handle_exception(val)?;
            Ok(String::from_js_value(self.ctx.clone(), val))
        }
    }

    /// Convert the atom to a JavaScript value, which will be either a [String] or a [Symbol][crate::Symbol].
    pub fn to_value(&self) -> Result<Value<'js>> {
        unsafe {
            let val = qjs::JS_AtomToValue(self.ctx.as_ptr(), self.atom);
            let val = self.ctx.handle_exception(val)?;
            Ok(Value::from_js_value(self.ctx.clone(), val))
        }
    }

    pub(crate) unsafe fn from_atom_val(ctx: Ctx<'js>, val: qjs::JSAtom) -> Self {
        Atom { atom: val, ctx }
    }

    pub(crate) unsafe fn from_atom_val_dup(ctx: Ctx<'js>, val: qjs::JSAtom) -> Self {
        qjs::JS_DupAtom(ctx.as_ptr(), val);
        Atom { atom: val, ctx }
    }
}

impl<'js> Clone for Atom<'js> {
    fn clone(&self) -> Atom<'js> {
        let atom = unsafe { qjs::JS_DupAtom(self.ctx.as_ptr(), self.atom) };
        Atom {
            atom,
            ctx: self.ctx.clone(),
        }
    }
}

impl<'js> Drop for Atom<'js> {
    fn drop(&mut self) {
        unsafe {
            qjs::JS_FreeAtom(self.ctx.as_ptr(), self.atom);
        }
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    #[test]
    fn to_string() {
        test_with(|ctx| {
            for text in ["", "hello", "hello\0😀"] {
                let atom = Atom::from_str(ctx.clone(), text).unwrap();
                assert_eq!(atom.to_string().unwrap(), text);
            }
            let atom = Atom::from_u32(ctx.clone(), 123).unwrap();
            assert_eq!(atom.to_string().unwrap(), "123");

            for source in [r#""\ud800""#, r#""\udfff""#] {
                let value: Value = ctx.eval(source).unwrap();
                let atom = Atom::from_value(ctx.clone(), &value).unwrap();
                assert!(matches!(atom.to_string(), Err(Error::Utf8(_))));
            }
        });
    }

    #[test]
    fn to_string_lossy() {
        test_with(|ctx| {
            for text in ["", "hello", "hello\0😀"] {
                let atom = Atom::from_str(ctx.clone(), text).unwrap();
                assert_eq!(atom.to_string_lossy().unwrap(), text);
            }
            let atom = Atom::from_u32(ctx.clone(), 123).unwrap();
            assert_eq!(atom.to_string_lossy().unwrap(), "123");

            for source in [r#""\ud800""#, r#""\udfff""#] {
                let value: Value = ctx.eval(source).unwrap();
                let atom = Atom::from_value(ctx.clone(), &value).unwrap();
                assert_eq!(atom.to_string_lossy().unwrap(), "���");
            }
        });
    }

    #[test]
    fn to_value() {
        test_with(|ctx| {
            let local_symbol = Symbol::new(ctx.clone()).unwrap();
            for (atom, expected) in [
                (
                    Atom::from_bool(ctx.clone(), false).unwrap(),
                    String::from_str(ctx.clone(), "false").unwrap().into(),
                ),
                (
                    Atom::from_u32(ctx.clone(), 123).unwrap(),
                    String::from_str(ctx.clone(), "123").unwrap().into(),
                ),
                (
                    Atom::from_f64(ctx.clone(), 123.4).unwrap(),
                    String::from_str(ctx.clone(), "123.4").unwrap().into(),
                ),
                (
                    Atom::from_predefined(ctx.clone(), atom::PredefinedAtom::New),
                    String::from_str(ctx.clone(), "new").unwrap().into(),
                ),
                (
                    Atom::from_predefined(ctx.clone(), atom::PredefinedAtom::SymbolToStringTag),
                    ctx.eval("Symbol.toStringTag").unwrap(),
                ),
                (
                    Atom::from_value(ctx.clone(), &local_symbol).unwrap(),
                    local_symbol.into(),
                ),
            ] {
                let to_value = atom.to_value().unwrap();
                assert!(
                    to_value.strict_eq(&expected),
                    "{to_value:?} != {expected:?}",
                );
            }
        });
    }
}
