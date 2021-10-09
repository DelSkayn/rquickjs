use crate::{AttributeArgs, Config, Ident, Parenthesized, PubVis};
use darling::{util::Override, FromMeta};
use syn::{parse2, AttrStyle, Attribute, Path};

/// Root binding item attrs
#[derive(Default, FromMeta, Debug, PartialEq, Eq)]
#[darling(default)]
pub struct AttrItem {
    /// Module name for export
    pub ident: Option<Ident>,
    /// Create module init function
    pub init: Option<Override<Ident>>,
    /// Export as module via ModuleDef
    pub module: bool,
    /// Export as object via ObjectDef
    pub object: bool,
    /// Test export
    pub test: bool,
    /// Export data visibility
    pub public: Option<Override<PubVis>>,
    /// Binding attribute name (`quickjs` by default)
    pub bind: Option<Ident>,
    /// Library crate name (determined automatically, usually `rquickjs`)
    #[darling(rename = "crate")]
    pub crate_: Option<Ident>,
    /// Exports variable name (`exports` by default)
    pub exports: Option<Ident>,
}

impl AttrItem {
    pub fn config(&self) -> Config {
        let mut cfg = Config::new();
        if let Some(crate_) = &self.crate_ {
            cfg.lib_crate = crate_.clone();
        }
        if let Some(bind) = &self.bind {
            cfg.bind_attr = bind.clone();
        }
        if let Some(exports) = &self.exports {
            cfg.exports_var = exports.clone();
        }
        cfg
    }
}

/// Module attrs
#[derive(Default, FromMeta, Debug, PartialEq, Eq)]
#[darling(default)]
pub struct AttrMod {
    /// Module name for export
    #[darling(rename = "rename")]
    pub name: Option<String>,
    /// Bare module export
    pub bare: bool,
    /// Skip export
    pub skip: bool,
    /// Do not output
    pub hide: bool,
}

impl Extend<Self> for AttrMod {
    fn extend<T: IntoIterator<Item = Self>>(&mut self, iter: T) {
        for over in iter {
            if over.name.is_some() {
                self.name = over.name;
            }
            if over.bare {
                self.bare = true;
            }
            if over.skip {
                self.skip = true;
            }
            if over.hide {
                self.hide = true;
            }
        }
    }
}

/// Constant attrs
#[derive(Default, FromMeta, Debug, PartialEq, Eq)]
#[darling(default)]
pub struct AttrVar {
    /// Variable name for export
    #[darling(rename = "rename")]
    pub name: Option<String>,
    /// Defines a property
    #[darling(rename = "value")]
    pub prop: bool,
    /// Create writable property
    pub writable: bool,
    /// Create configurable property
    pub configurable: bool,
    /// Create enumerable property
    pub enumerable: bool,
    /// Set to prototype
    pub proto: bool,
    /// Skip export
    pub skip: bool,
    /// Do not output
    pub hide: bool,
}

impl Extend<Self> for AttrVar {
    fn extend<T: IntoIterator<Item = Self>>(&mut self, iter: T) {
        for over in iter {
            if over.name.is_some() {
                self.name = over.name;
            }
            if over.prop {
                self.prop = true;
            }
            if over.writable {
                self.writable = true;
            }
            if over.configurable {
                self.configurable = true;
            }
            if over.enumerable {
                self.enumerable = true;
            }
            if over.proto {
                self.proto = true;
            }
            if over.skip {
                self.skip = true;
            }
            if over.hide {
                self.hide = true;
            }
        }
    }
}

/// Function attrs
#[derive(Default, FromMeta, Debug, PartialEq, Eq)]
#[darling(default)]
pub struct AttrFn {
    /// Function name for export
    #[darling(rename = "rename")]
    pub name: Option<String>,
    /// Use as getter for specified property
    pub get: bool,
    /// Use as setter for specified property
    pub set: bool,
    /// Create configurable property
    pub configurable: bool,
    /// Create enumerable property
    pub enumerable: bool,
    /// Use as constructor
    #[darling(rename = "constructor")]
    pub ctor: Override<bool>,
    /// Skip export
    pub skip: bool,
    /// Do not output
    pub hide: bool,
}

impl Extend<Self> for AttrFn {
    fn extend<T: IntoIterator<Item = Self>>(&mut self, iter: T) {
        for over in iter {
            if over.name.is_some() {
                self.name = over.name;
            }
            if over.get {
                self.get = true;
            }
            if over.set {
                self.set = true;
            }
            if over.configurable {
                self.configurable = true;
            }
            if over.enumerable {
                self.enumerable = true;
            }
            if over.skip {
                self.skip = true;
            }
            if over.hide {
                self.hide = true;
            }
        }
    }
}

/// Data attrs
#[derive(Default, FromMeta, Debug, PartialEq, Eq)]
#[darling(default)]
pub struct AttrData {
    /// Data name for export
    #[darling(rename = "rename")]
    pub name: Option<String>,
    /// Data has internal refs
    pub has_refs: bool,
    /// Data implements [`Clone`] trait
    pub cloneable: bool,
    /// Skip export
    pub skip: bool,
    /// Do not output
    pub hide: bool,
}

impl Extend<Self> for AttrData {
    fn extend<T: IntoIterator<Item = Self>>(&mut self, iter: T) {
        for over in iter {
            if over.name.is_some() {
                self.name = over.name;
            }
            if over.has_refs {
                self.has_refs = true;
            }
            if over.cloneable {
                self.cloneable = true;
            }
            if over.skip {
                self.skip = true;
            }
            if over.hide {
                self.hide = true;
            }
        }
    }
}

/// Data field attrs
#[derive(Default, FromMeta, Debug, PartialEq, Eq)]
#[darling(default)]
pub struct AttrField {
    /// Variable name for export
    #[darling(rename = "rename")]
    pub name: Option<String>,
    /// Readonly property
    pub readonly: bool,
    /// Skip export
    pub skip: bool,
}

impl Extend<Self> for AttrField {
    fn extend<T: IntoIterator<Item = Self>>(&mut self, iter: T) {
        for over in iter {
            if over.name.is_some() {
                self.name = over.name;
            }
            if over.readonly {
                self.readonly = true;
            }
            if over.skip {
                self.skip = true;
            }
        }
    }
}

/// Impl attrs
#[derive(Default, FromMeta, Debug, PartialEq, Eq)]
#[darling(default)]
pub struct AttrImpl {
    /// Related class name
    #[darling(rename = "rename")]
    pub name: Option<String>,
    /// Data has internal refs
    pub has_refs: bool,
    /// Data implements [`Clone`] trait
    pub cloneable: bool,
    /// Skip export
    pub skip: bool,
    /// Do not output
    pub hide: bool,
}

impl Extend<Self> for AttrImpl {
    fn extend<T: IntoIterator<Item = Self>>(&mut self, iter: T) {
        for over in iter {
            if over.name.is_some() {
                self.name = over.name;
            }
            if over.has_refs {
                self.has_refs = true;
            }
            if over.cloneable {
                self.cloneable = true;
            }
            if over.skip {
                self.skip = true;
            }
            if over.hide {
                self.hide = true;
            }
        }
    }
}

fn is_attr(
    ident: &Ident,
    style: &AttrStyle,
    Path {
        leading_colon,
        segments,
    }: &Path,
) -> bool {
    style == &AttrStyle::Outer
        && leading_colon.is_none()
        && segments.len() == 1
        && &segments[0].ident == ident
}

pub fn get_attrs<R: FromMeta + Default + Extend<R>>(
    ident: &Ident,
    attrs: &mut Vec<Attribute>,
) -> R {
    let mut res = R::default();

    attrs.retain(
        |Attribute {
             style,
             path,
             tokens,
             ..
         }| {
            if is_attr(ident, style, path) {
                match parse2(tokens.clone()).map(
                    |Parenthesized {
                         content: AttributeArgs(attrs),
                         ..
                     }| FromMeta::from_list(&attrs),
                ) {
                    Ok(Ok(val)) => res.extend(Some(val).into_iter()),
                    Ok(Err(error)) => warning!("{}", error),
                    Err(error) => warning!("{}", error),
                }
                false
            } else {
                true
            }
        },
    );

    res
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
        attr_item {
            empty [] AttrItem::default();

            init_default [init] AttrItem {
                init: Some(Override::Inherit),
                ..Default::default()
            };

            init_named [init = "js_init_awesome_module"] AttrItem {
                init: Some(Override::Explicit(quote::format_ident!(
                    "js_init_awesome_module"
                ))),
                ..Default::default()
            };

            public_default [public] AttrItem {
                public: Some(Override::Inherit),
                ..Default::default()
            };

            public_restricted [public = "crate"] AttrItem {
                public: Some(Override::Explicit(PubVis::Crate)),
                ..Default::default()
            };
        }

        attr_mod {
            empty [] AttrMod::default();

            rename [rename = "new"] AttrMod {
                name: Some("new".into()),
                ..Default::default()
            };

            bare [bare] AttrMod {
                bare: true,
                ..Default::default()
            };

            skip [skip] AttrMod {
                skip: true,
                ..Default::default()
            };

            all [rename = "new", bare, skip] AttrMod {
                name: Some("new".into()),
                bare: true,
                skip: true,
                hide: false,
            };

            #[should_panic(expected = "Unknown field: `some`")]
            unknown_field [some = "other"] AttrMod::default();

            #[should_panic(expected = "Unknown literal value `some` at bare")]
            unexpected_value [bare = "some"] AttrMod::default();
        }

        attr_var {
            empty [] AttrVar::default();

            rename [rename = "new"] AttrVar {
                name: Some("new".into()),
                ..Default::default()
            };

            property [value] AttrVar {
                prop: true,
                ..Default::default()
            };

            skip [skip] AttrVar {
                skip: true,
                ..Default::default()
            };

            all [rename = "new", value, skip] AttrVar {
                name: Some("new".into()),
                prop: true,
                skip: true,
                ..Default::default()
            };

            #[should_panic(expected = "Unknown field: `some`")]
            unknown_field [some = "other"] AttrVar::default();

            #[should_panic(expected = "Unknown literal value `some` at value")]
            unexpected_value [value = "some"] AttrVar::default();
        }

        attr_fn {
            empty [] AttrFn::default();

            rename [rename = "new"] AttrFn {
                name: Some("new".into()),
                ..Default::default()
            };

            getter [get] AttrFn {
                get: true,
                ..Default::default()
            };

            setter [set] AttrFn {
                set: true,
                ..Default::default()
            };

            skip [skip] AttrFn {
                skip: true,
                ..Default::default()
            };

            all [rename = "new", get, set, skip] AttrFn {
                name: Some("new".into()),
                get: true,
                set: true,
                skip: true,
                ..Default::default()
            };

            #[should_panic(expected = "Unknown field: `some`")]
            unknown_field [some = "other"] AttrFn::default();

            #[should_panic(expected = "Unexpected literal type `bool` at rename")]
            unexpected_value [rename = true] AttrFn::default();
        }
    }
}
