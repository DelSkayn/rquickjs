use crate::{Expander, Tokens};
use quote::quote;
use syn::{Ident, ItemConst};

impl Expander {
    /// Expand constant
    pub fn constant(&self, path: &Vec<&Ident>, ItemConst { ident, .. }: &ItemConst) -> Tokens {
        let exports = &self.exports;

        let name = format!("{}", ident);
        let path = self.path(path, ident);

        quote! {
            #exports.set(#name, #path)?;
        }
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use quote::quote;
    use syn::parse_quote;

    #[test]
    fn const_num() {
        let item = parse_quote! {
            const PI: f32 = core::f32::consts::PI;
        };

        let expander = Expander::new();
        let path = Vec::new();

        let actual = expander.constant(&path, &item);
        let expected = quote! {
            exports.set("PI", PI)?;
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }
}
