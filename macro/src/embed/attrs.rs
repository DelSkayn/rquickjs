use crate::{Config, Ident, PubVis};
use darling::{util::Override, FromMeta};

/// The embed macro attrs
#[derive(Default, FromMeta, Debug, PartialEq, Eq)]
#[darling(default)]
pub struct AttrEmbed {
    /// Module name for export
    pub ident: Option<Ident>,
    /// Module search paths
    #[darling(rename = "path", multiple)]
    pub paths: Vec<String>,
    /// Module patterns
    #[darling(rename = "pattern", multiple)]
    pub patterns: Vec<String>,
    /// Module names to embed
    #[darling(rename = "name", multiple)]
    pub names: Vec<String>,
    #[cfg(feature = "phf")]
    /// Use perfect hash for module map
    #[darling(rename = "perfect")]
    pub phf_map: bool,
    /// Enable test mode (skip bytecode)
    pub test: bool,
    /// Struct visibility
    pub public: Option<Override<PubVis>>,
    /// Library crate name (determined automatically, usually `rquickjs`)
    #[darling(rename = "crate")]
    pub crate_: Option<Ident>,
}

impl AttrEmbed {
    pub fn config(&self) -> Config {
        let mut cfg = Config::new();
        if let Some(crate_) = &self.crate_ {
            cfg.lib_crate = crate_.clone();
        }
        cfg
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn force_type_infer<T>(_a: &T, _b: &T) {}

    macro_rules! tests {
        ($($attr:ident { $($(#[$meta:meta])* $test:ident [$($input:tt)*] $expected:expr;)* })*) => {
            $(
                mod $attr {
                    use super::*;

                    $(
                        #[test]
                        $(#[$meta])*
                        fn $test() {
                            let attr: crate::AttributeArgs = syn::parse_quote! { $($input)* };
                            let actual = darling::FromMeta::from_list(&attr).map_err(|error| panic!("{}", error)).unwrap();
                            let expected = $expected;
                            force_type_infer(&expected, &actual);
                            assert_eq!(actual, expected);
                        }
                    )*
                }
            )*
        };
    }

    tests! {
        attr {
            empty [] AttrEmbed::default();

            with_name [name = "my_module"] AttrEmbed {
                names: vec!["my_module".into()],
                ..Default::default()
            };

            with_path [path = "./my_modules"] AttrEmbed {
                paths: vec!["./my_modules".into()],
                ..Default::default()
            };

            with_paths [path = "./my_modules", path = "../other/modules"] AttrEmbed {
                paths: vec!["./my_modules".into(), "../other/modules".into()],
                ..Default::default()
            };

            with_pattern [pattern = "{}.js"] AttrEmbed {
                patterns: vec!["{}.js".into()],
                ..Default::default()
            };

            with_patterns [pattern = "{}.js", pattern = "{}.mjs"] AttrEmbed {
                patterns: vec!["{}.js".into(), "{}.mjs".into()],
                ..Default::default()
            };
        }
    }
}
