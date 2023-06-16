use crate::{qjs, Atom, Result, String, Value};

/// Rust representation of a javascript symbol.
#[derive(Debug, Clone, PartialEq)]
#[repr(transparent)]
pub struct Symbol<'js>(pub(crate) Value<'js>);

impl<'js> Symbol<'js> {
    /// Get the symbol description
    pub fn description(&self) -> Result<String<'js>> {
        let atom = Atom::from_str(self.0.ctx, "description")?;
        unsafe {
            let val = qjs::JS_GetProperty(self.0.ctx.as_ptr(), self.0.as_js_value(), atom.atom);
            let val = self.0.ctx.handle_exception(val)?;
            Ok(String::from_js_value(self.0.ctx, val))
        }
    }
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
