use crate::{handle_exception, qjs, Ctx, Error, Result, String, Value};
use std::{ffi::CStr, mem, string::String as StdString};

/// An atom is value representing the name of a variable of an objects and can be created
/// from any javascript value.
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
    pub fn from_value(ctx: Ctx<'js>, val: &Value<'js>) -> Atom<'js> {
        // TODO figure out if this can give errors
        // It seems like it could but I have not yet figured out
        // how to detect this.
        let atom = unsafe { qjs::JS_ValueToAtom(ctx.ctx, val.as_js_value()) };
        Atom { atom, ctx }
    }

    /// Create a atom from a u32
    pub fn from_u32(ctx: Ctx<'js>, val: u32) -> Atom<'js> {
        // TODO figure out if this can give errors
        // It seems like it could but I have not yet figured out
        // how to detect this.
        let atom = unsafe { qjs::JS_NewAtomUInt32(ctx.ctx, val) };
        Atom { atom, ctx }
    }

    /// Create a atom from an i32 via value
    pub fn from_i32(ctx: Ctx<'js>, val: i32) -> Atom<'js> {
        // TODO figure out if this can give errors
        // It seems like it could but I have not yet figured out
        // how to detect this.
        let atom = unsafe { qjs::JS_ValueToAtom(ctx.ctx, qjs::JS_MKVAL(qjs::JS_TAG_INT, val)) };
        Atom { atom, ctx }
    }

    /// Create a atom from a bool via value
    pub fn from_bool(ctx: Ctx<'js>, val: bool) -> Atom<'js> {
        // TODO figure out if this can give errors
        // It seems like it could but I have not yet figured out
        // how to detect this.
        let val = if val { qjs::JS_TRUE } else { qjs::JS_FALSE };
        let atom = unsafe { qjs::JS_ValueToAtom(ctx.ctx, val) };
        Atom { atom, ctx }
    }

    /// Create a atom from a f64 via value
    pub fn from_f64(ctx: Ctx<'js>, val: f64) -> Atom<'js> {
        // TODO figure out if this can give errors
        // It seems like it could but I have not yet figured out
        // how to detect this.
        let atom = unsafe { qjs::JS_ValueToAtom(ctx.ctx, qjs::JS_NewFloat64(val)) };
        Atom { atom, ctx }
    }

    /// Create a atom from a rust string
    pub fn from_str(ctx: Ctx<'js>, name: &str) -> Atom<'js> {
        // TODO figure out if this can give errors
        // It seems like it could but I have not yet figured out
        // how to detect this.
        unsafe {
            let ptr = name.as_ptr() as *const std::os::raw::c_char;
            let atom = qjs::JS_NewAtomLen(ctx.ctx, ptr, name.len() as _);
            Atom { atom, ctx }
        }
    }

    /// Convert the atom to a javascript string.
    pub fn to_string(&self) -> Result<StdString> {
        pub struct DropStr<'js>(Ctx<'js>, *const std::os::raw::c_char);

        impl<'js> Drop for DropStr<'js> {
            fn drop(&mut self) {
                unsafe {
                    qjs::JS_FreeCString(self.0.ctx, self.1);
                }
            }
        }

        unsafe {
            let c_str = qjs::JS_AtomToCString(self.ctx.ctx, self.atom);
            // Ensure the c_string is dropped no matter what happens
            let drop = DropStr(self.ctx, c_str);
            if c_str.is_null() {
                // Might not ever happen but I am not 100% sure
                // so just incase check it.
                return Err(Error::Unknown);
            }
            let res = CStr::from_ptr(c_str).to_str()?.to_string();
            mem::drop(drop);
            Ok(res)
        }
    }

    /// Convert the atom to a javascript string .
    pub fn to_js_string(&self) -> Result<String<'js>> {
        unsafe {
            let val = qjs::JS_AtomToString(self.ctx.ctx, self.atom);
            let val = handle_exception(self.ctx, val)?;
            Ok(String::from_js_value(self.ctx, val))
        }
    }

    /// Convert the atom to a javascript value.
    pub fn to_value(&self) -> Result<Value<'js>> {
        self.to_js_string().map(|String(value)| value)
    }

    /// Create an atom from raw quickjs atom value.
    #[doc(hidden)]
    pub unsafe fn from_atom_val(ctx: Ctx<'js>, val: qjs::JSAtom) -> Self {
        Atom { atom: val, ctx }
    }
}

impl<'js> Clone for Atom<'js> {
    fn clone(&self) -> Atom<'js> {
        let atom = unsafe { qjs::JS_DupAtom(self.ctx.ctx, self.atom) };
        Atom {
            atom,
            ctx: self.ctx,
        }
    }
}

impl<'js> Drop for Atom<'js> {
    fn drop(&mut self) {
        unsafe {
            qjs::JS_FreeAtom(self.ctx.ctx, self.atom);
        }
    }
}
