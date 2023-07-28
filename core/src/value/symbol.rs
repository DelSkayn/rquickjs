use crate::{qjs, Atom, Ctx, Result, String, Value};

/// Rust representation of a javascript symbol.
#[derive(Debug, Clone, PartialEq, Hash)]
#[repr(transparent)]
pub struct Symbol<'js>(pub(crate) Value<'js>);

impl<'js> Symbol<'js> {
    /// Get the symbol description
    pub fn description(&self) -> Result<String<'js>> {
        let atom = Atom::from_str(self.0.ctx.clone(), "description")?;
        unsafe {
            let val = qjs::JS_GetProperty(self.0.ctx.as_ptr(), self.0.as_js_value(), atom.atom);
            let val = self.0.ctx.handle_exception(val)?;
            Ok(String::from_js_value(self.0.ctx.clone(), val))
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
    /// returns the symbol for `operatorSet`
    operator_set => JS_ATOM_Symbol_operatorSet
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn description() {
        test_with(|ctx| {
            let s: Symbol<'_> = ctx.eval("Symbol('foo bar baz')").unwrap();
            assert_eq!(s.description().unwrap().to_string().unwrap(), "foo bar baz");

            let s: Symbol<'_> = ctx.eval("Symbol()").unwrap();
            assert_eq!(s.description().unwrap().to_string().unwrap(), "undefined");
        });
    }
}
