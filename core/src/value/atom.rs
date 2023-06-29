use crate::{qjs, Ctx, Error, Result, String, Value};
use std::{ffi::CStr, string::String as StdString};

/// A collection of atoms which are predefined in quickjs.
///
/// Using these over [`Atom::from_str`] can be more performant as these don't need to be looked up
/// in a hashmap.
#[repr(u32)]
pub enum PredefinedAtoms {
    /// "length"
    Length = qjs::JS_ATOM_length,
    /// "fileName",
    FileName = qjs::JS_ATOM_fileName,
    /// "LineNumber",
    LineNumber = qjs::JS_ATOM_lineNumber,
    /// "stack",
    Stack = qjs::JS_ATOM_stack,
    /// "name",
    Name = qjs::JS_ATOM_name,
    /// "toString",
    ToString = qjs::JS_ATOM_toString,
    /// "valueOf",
    ValueOf = qjs::JS_ATOM_valueOf,
    /// "prototype",
    Prototype = qjs::JS_ATOM_prototype,
    /// "constructor",
    Constructor = qjs::JS_ATOM_constructor,
    /// "configurable",
    Configurable = qjs::JS_ATOM_configurable,
    /// "writable",
    Writable = qjs::JS_ATOM_writable,
    /// "enumerable",
    Enumerable = qjs::JS_ATOM_enumerable,
    /// "Symbol.iterator"
    SymbolIterator = qjs::JS_ATOM_Symbol_iterator,
    /// "Symbol.match"
    SymbolMatch = qjs::JS_ATOM_Symbol_match,
    /// "Symbol.matchAll"
    SymbolMatchAll = qjs::JS_ATOM_Symbol_matchAll,
    /// "Symbol.replace"
    SymbolReplace = qjs::JS_ATOM_Symbol_replace,
    /// "Symbol.search"
    SymbolSearch = qjs::JS_ATOM_Symbol_search,
    /// "Symbol.split"
    SymbolSplit = qjs::JS_ATOM_Symbol_split,
    /// "Symbol.toStringTag"
    SymbolToStringTag = qjs::JS_ATOM_Symbol_toStringTag,
    /// "Symbol.isConcatSpreadable"
    SymbolIsConcatSpreadable = qjs::JS_ATOM_Symbol_isConcatSpreadable,
    /// "Symbol.hasInstance"
    SymbolHasInstance = qjs::JS_ATOM_Symbol_hasInstance,
    /// "Symbol.species"
    SymbolSpecies = qjs::JS_ATOM_Symbol_species,
    /// "Symbol.unscopables"
    SymbolUnscopables = qjs::JS_ATOM_Symbol_unscopables,
}

///
/// # Representation
///
/// Atoms in quickjs are handled differently depending on what type of index the represent.
/// When the atom represents a number like index, like `object[1]` the atom is just
/// a normal number.
/// However when the atom represents a string link index like `object["foo"]` or `object.foo`
/// the atom represents a value in a hashmap.
pub struct Atom<'js> {
    pub(crate) atom: qjs::JSAtom,
    ctx: Ctx<'js>,
}

impl<'js> Atom<'js> {
    /// Create a atom from a javascript value.
    pub fn from_value(ctx: Ctx<'js>, val: &Value<'js>) -> Result<Atom<'js>> {
        let atom = unsafe { qjs::JS_ValueToAtom(ctx.as_ptr(), val.as_js_value()) };
        if atom == qjs::JS_ATOM_NULL {
            // A value can be anything, including an object which might contain a callback so check
            // for panics.
            return Err(ctx.raise_exception());
        }
        Ok(Atom { atom, ctx })
    }

    /// Create a atom from a u32
    pub fn from_u32(ctx: Ctx<'js>, val: u32) -> Result<Atom<'js>> {
        let atom = unsafe { qjs::JS_NewAtomUInt32(ctx.as_ptr(), val) };
        if atom == qjs::JS_ATOM_NULL {
            // Should never invoke a callback so no panics
            return Err(Error::Exception);
        }
        Ok(Atom { atom, ctx })
    }

    /// Create a atom from an i32 via value
    pub fn from_i32(ctx: Ctx<'js>, val: i32) -> Result<Atom<'js>> {
        let atom =
            unsafe { qjs::JS_ValueToAtom(ctx.as_ptr(), qjs::JS_MKVAL(qjs::JS_TAG_INT, val)) };
        if atom == qjs::JS_ATOM_NULL {
            // Should never invoke a callback so no panics
            return Err(Error::Exception);
        }
        Ok(Atom { atom, ctx })
    }

    /// Create a atom from a bool via value
    pub fn from_bool(ctx: Ctx<'js>, val: bool) -> Result<Atom<'js>> {
        let val = if val { qjs::JS_TRUE } else { qjs::JS_FALSE };
        let atom = unsafe { qjs::JS_ValueToAtom(ctx.as_ptr(), val) };
        if atom == qjs::JS_ATOM_NULL {
            // Should never invoke a callback so no panics
            return Err(Error::Exception);
        }
        Ok(Atom { atom, ctx })
    }

    /// Create a atom from a f64 via value
    pub fn from_f64(ctx: Ctx<'js>, val: f64) -> Result<Atom<'js>> {
        let atom = unsafe { qjs::JS_ValueToAtom(ctx.as_ptr(), qjs::JS_NewFloat64(val)) };
        if atom == qjs::JS_ATOM_NULL {
            // Should never invoke a callback so no panics
            return Err(Error::Exception);
        }
        Ok(Atom { atom, ctx })
    }

    /// Create a atom from a rust string
    pub fn from_str(ctx: Ctx<'js>, name: &str) -> Result<Atom<'js>> {
        unsafe {
            let ptr = name.as_ptr() as *const std::os::raw::c_char;
            let atom = qjs::JS_NewAtomLen(ctx.as_ptr(), ptr, name.len() as _);
            if atom == qjs::JS_ATOM_NULL {
                // Should never invoke a callback so no panics
                return Err(Error::Exception);
            }
            Ok(Atom { atom, ctx })
        }
    }

    /// Convert the atom to a javascript string.
    pub fn to_string(&self) -> Result<StdString> {
        unsafe {
            let c_str = qjs::JS_AtomToCString(self.ctx.as_ptr(), self.atom);
            if c_str.is_null() {
                // Might not ever happen but I am not 100% sure
                // so just incase check it.
                qjs::JS_FreeCString(self.ctx.as_ptr(), c_str);
                return Err(Error::Unknown);
            }
            let bytes = CStr::from_ptr(c_str).to_bytes();
            // Safety: quickjs should return valid utf8 so this should be safe.
            let res = std::str::from_utf8_unchecked(bytes).to_string();
            qjs::JS_FreeCString(self.ctx.as_ptr(), c_str);
            Ok(res)
        }
    }

    /// Convert the atom to a javascript string .
    pub fn to_js_string(&self) -> Result<String<'js>> {
        unsafe {
            let val = qjs::JS_AtomToString(self.ctx.as_ptr(), self.atom);
            let val = self.ctx.handle_exception(val)?;
            Ok(String::from_js_value(self.ctx, val))
        }
    }

    /// Convert the atom to a javascript value.
    pub fn to_value(&self) -> Result<Value<'js>> {
        self.to_js_string().map(|String(value)| value)
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
            ctx: self.ctx,
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
