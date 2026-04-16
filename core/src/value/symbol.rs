use alloc::{ffi::CString, vec::Vec};

use crate::{qjs, Atom, Ctx, Result, Value};

/// Rust representation of a JavaScript symbol.
#[derive(Debug, Clone, PartialEq, Hash)]
#[repr(transparent)]
pub struct Symbol<'js>(pub(crate) Value<'js>);

impl<'js> Symbol<'js> {
    fn new_inner(
        ctx: Ctx<'js>,
        description: Option<impl Into<Vec<u8>>>,
        is_global: bool,
    ) -> Result<Self> {
        let c_str = description.map(CString::new).transpose()?;
        let ptr = c_str.as_ref().map_or(core::ptr::null(), |s| s.as_ptr());
        unsafe {
            let val = qjs::JS_NewSymbol(ctx.as_ptr(), ptr, is_global);
            let val = ctx.handle_exception(val)?;
            Ok(Symbol(Value::from_js_value(ctx, val)))
        }
    }

    /// Create a new unique local symbol without a description (equivalent to `Symbol()`).
    pub fn new(ctx: Ctx<'js>) -> Result<Self> {
        Self::new_inner(ctx, None::<&str>, false)
    }

    /// Create a new unique local symbol with a description (equivalent to `Symbol(description)`).
    pub fn with_description(ctx: Ctx<'js>, description: impl Into<Vec<u8>>) -> Result<Self> {
        Self::new_inner(ctx, Some(description), false)
    }

    /// Create or retrieve a global symbol for the given key (equivalent to `Symbol.for(key)`).
    pub fn new_global(ctx: Ctx<'js>, description: impl Into<Vec<u8>>) -> Result<Self> {
        Self::new_inner(ctx, Some(description), true)
    }

    /// Get the symbol description
    pub fn description(&self) -> Result<Value<'js>> {
        let atom = Atom::from_str(self.0.ctx.clone(), "description")?;
        unsafe {
            let val = qjs::JS_GetProperty(self.0.ctx.as_ptr(), self.0.as_js_value(), atom.atom);
            let val = self.0.ctx.handle_exception(val)?;
            Ok(Value::from_js_value(self.0.ctx.clone(), val))
        }
    }

    /// Convert a symbol into a atom.
    pub fn as_atom(&self) -> Atom<'js> {
        Atom::from_value(self.0.ctx().clone(), &self.0)
            .expect("symbols should always convert to atoms")
    }
}

macro_rules! impl_symbols {
    ($($(#[$m:meta])? $fn_name:ident => $const_name:ident)*) => {
        impl<'js> Symbol<'js> {
            $(
            $(#[$m])*
            pub fn $fn_name(ctx: Ctx<'js>) -> Self {
                // No-op in most cases but with certain dump flags static symbols maintain a ref count.
                let v = unsafe {
                    let v = qjs::JS_AtomToValue(ctx.as_ptr(),qjs::$const_name as qjs::JSAtom);
                    Value::from_js_value(ctx, v)
                };

                v.into_symbol().unwrap()
            }
            )*
        }
    };
}

impl_symbols! {
    /// returns the symbol for `toPrimitive`
    to_primitive => JS_ATOM_Symbol_toPrimitive
    /// returns the symbol for `iterator`
    iterator => JS_ATOM_Symbol_iterator
    /// returns the symbol for `match`
    r#match => JS_ATOM_Symbol_match
    /// returns the symbol for `matchAll`
    match_all => JS_ATOM_Symbol_matchAll
    /// returns the symbol for `replace`
    replace => JS_ATOM_Symbol_replace
    /// returns the symbol for `search`
    search => JS_ATOM_Symbol_search
    /// returns the symbol for `split`
    split => JS_ATOM_Symbol_split
    /// returns the symbol for `hasInstance`
    has_instance => JS_ATOM_Symbol_hasInstance
    /// returns the symbol for `species`
    species => JS_ATOM_Symbol_species
    /// returns the symbol for `unscopables`
    unscopables => JS_ATOM_Symbol_unscopables
    /// returns the symbol for `asyncIterator`
    async_iterator => JS_ATOM_Symbol_asyncIterator
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn description() {
        test_with(|ctx| {
            let s: Symbol<'_> = ctx.eval("Symbol('foo bar baz')").unwrap();
            assert_eq!(
                s.description()
                    .unwrap()
                    .into_string()
                    .unwrap()
                    .to_string()
                    .unwrap(),
                "foo bar baz"
            );

            let s: Symbol<'_> = ctx.eval("Symbol()").unwrap();
            assert!(s.description().unwrap().is_undefined());
        });
    }

    #[test]
    fn new_without_description() {
        test_with(|ctx| {
            let s = Symbol::new(ctx).unwrap();
            assert!(s.description().unwrap().is_undefined());
        });
    }

    #[test]
    fn new_with_description() {
        test_with(|ctx| {
            let s = Symbol::with_description(ctx, "test").unwrap();
            assert_eq!(
                s.description()
                    .unwrap()
                    .into_string()
                    .unwrap()
                    .to_string()
                    .unwrap(),
                "test"
            );
        });
    }

    #[test]
    fn new_unique() {
        test_with(|ctx| {
            let a = Symbol::with_description(ctx.clone(), "same").unwrap();
            let b = Symbol::with_description(ctx, "same").unwrap();
            assert_ne!(a, b);
        });
    }

    #[test]
    fn new_global() {
        test_with(|ctx| {
            let a = Symbol::new_global(ctx.clone(), "shared").unwrap();
            let b = Symbol::new_global(ctx.clone(), "shared").unwrap();
            assert_eq!(a, b);

            // Should also match Symbol.for() from JS
            let c: Symbol<'_> = ctx.eval("Symbol.for('shared')").unwrap();
            assert_eq!(a, c);
        });
    }
}
