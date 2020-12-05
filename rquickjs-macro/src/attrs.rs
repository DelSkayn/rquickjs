use crate::{warning, AttributeArgs, Ident, Parenthesized};
use darling::FromMeta;
use syn::{parse2, AttrStyle, Attribute, Path};

pub trait Merge {
    fn merge(&mut self, over: Self);
}

/// Module specifier
#[derive(Debug, PartialEq, Eq)]
pub enum AttrItemModInit {
    Disabled,
    Enabled(Option<Ident>),
}

impl Default for AttrItemModInit {
    fn default() -> Self {
        Self::Disabled
    }
}

impl FromMeta for AttrItemModInit {
    fn from_word() -> darling::Result<Self> {
        Ok(Self::Enabled(None))
    }

    fn from_string(value: &str) -> darling::Result<Self> {
        Ok(Self::Enabled(Some(Ident::from_string(value)?)))
    }
}

/// Root binding item attrs
#[derive(Default, FromMeta, Debug, PartialEq, Eq)]
#[darling(default)]
pub struct AttrItem {
    /// Module name for export
    pub ident: Option<Ident>,
    /// Create module init function
    pub init: AttrItemModInit,
    /// Export as module via ModuleDef
    pub module: bool,
    /// Export as object via ObjectDef
    pub object: bool,
    /// Test export
    pub test: bool,
}

/// Module attrs
#[derive(Default, FromMeta, Debug, PartialEq, Eq)]
#[darling(default)]
pub struct AttrMod {
    /// Module name for export
    pub name: Option<String>,
    /// Bare module export
    pub bare: bool,
    /// Skip export
    pub skip: bool,
}

impl Merge for AttrMod {
    fn merge(&mut self, over: Self) {
        if over.name.is_some() {
            self.name = over.name;
        }
        if over.bare {
            self.bare = true;
        }
        if over.skip {
            self.skip = true;
        }
    }
}

/// Constant attrs
#[derive(Default, FromMeta, Debug, PartialEq, Eq)]
#[darling(default)]
pub struct AttrVar {
    /// Variable name for export
    pub name: Option<String>,
    /// Set as property
    pub prop: bool,
    /// Skip export
    pub skip: bool,
}

impl Merge for AttrVar {
    fn merge(&mut self, over: Self) {
        if over.name.is_some() {
            self.name = over.name;
        }
        if over.prop {
            self.prop = true;
        }
        if over.skip {
            self.skip = true;
        }
    }
}

/// Function attrs
#[derive(Default, FromMeta, Debug, PartialEq, Eq)]
#[darling(default)]
pub struct AttrFn {
    /// Function name for export
    pub name: Option<String>,
    /// Use as getter for specified property
    pub get: Option<String>,
    /// Use as setter for specified property
    pub set: Option<String>,
    /// Use as constructor
    pub ctor: bool,
    /// Skip export
    pub skip: bool,
}

impl Merge for AttrFn {
    fn merge(&mut self, over: Self) {
        if over.name.is_some() {
            self.name = over.name;
        }
        if over.get.is_some() {
            self.get = over.get;
        }
        if over.set.is_some() {
            self.set = over.set;
        }
        if over.skip {
            self.skip = true;
        }
    }
}

fn is_bind(
    style: &AttrStyle,
    Path {
        leading_colon,
        segments,
    }: &Path,
) -> bool {
    style == &AttrStyle::Outer
        && leading_colon.is_none()
        && segments.len() == 1
        && segments[0].ident == "bind"
}

pub fn get_attrs<R: FromMeta + Default + Merge>(attrs: &mut Vec<Attribute>) -> R {
    let mut res = R::default();

    attrs.retain(
        |Attribute {
             style,
             path,
             tokens,
             ..
         }| {
            if is_bind(style, path) {
                match parse2(tokens.clone()).map(
                    |Parenthesized {
                         content: AttributeArgs(attrs),
                         ..
                     }| FromMeta::from_list(&attrs),
                ) {
                    Ok(Ok(val)) => res.merge(val),
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

    macro_rules! parse {
        ($($x:tt)*) => {{
            let attr: crate::AttributeArgs = syn::parse_quote! { $($x)* };
            darling::FromMeta::from_list(&attr).map_err(|error| panic!("{}", error)).unwrap()
        }};
    }

    mod item_attrs {
        use super::*;

        #[test]
        fn empty() {
            let attr: AttrItem = parse! {};
            assert_eq!(attr, AttrItem::default());
        }

        #[test]
        fn init_default() {
            let attr: AttrItem = parse! { init };
            assert_eq!(
                attr,
                AttrItem {
                    init: AttrItemModInit::Enabled(None),
                    ..Default::default()
                }
            );
        }

        #[test]
        fn init_named() {
            let attr: AttrItem = parse! { init = "js_init_awesome_module" };
            assert_eq!(
                attr,
                AttrItem {
                    init: AttrItemModInit::Enabled(Some(quote::format_ident!(
                        "js_init_awesome_module"
                    ))),
                    ..Default::default()
                }
            );
        }
    }

    mod mod_attrs {
        use super::*;

        #[test]
        fn empty() {
            let attr: AttrMod = parse! {};
            assert_eq!(attr, AttrMod::default());
        }

        #[test]
        fn name() {
            let attr: AttrMod = parse! { name = "new" };
            assert_eq!(
                attr,
                AttrMod {
                    name: Some("new".into()),
                    ..Default::default()
                }
            );
        }

        #[test]
        fn bare() {
            let attr: AttrMod = parse! { bare };
            assert_eq!(
                attr,
                AttrMod {
                    bare: true,
                    ..Default::default()
                }
            );
        }

        #[test]
        fn skip() {
            let attr: AttrMod = parse! { skip };
            assert_eq!(
                attr,
                AttrMod {
                    skip: true,
                    ..Default::default()
                }
            );
        }

        #[test]
        fn all() {
            let attr: AttrMod = parse! { name = "new", bare, skip };
            assert_eq!(
                attr,
                AttrMod {
                    name: Some("new".into()),
                    bare: true,
                    skip: true,
                }
            );
        }

        #[test]
        #[should_panic(expected = "Unknown field: `some`")]
        fn unknown_field() {
            let _attr: AttrMod = parse! { some = "other" };
        }

        #[test]
        #[should_panic(expected = "Unknown literal value `some` at bare")]
        fn unexpected_value() {
            let _attr: AttrMod = parse! { bare = "some" };
        }
    }

    mod var_attrs {
        use super::*;

        #[test]
        fn empty() {
            let attr: AttrVar = parse! {};
            assert_eq!(attr, AttrVar::default());
        }

        #[test]
        fn name() {
            let attr: AttrVar = parse! { name = "new" };
            assert_eq!(
                attr,
                AttrVar {
                    name: Some("new".into()),
                    ..Default::default()
                }
            );
        }

        #[test]
        fn prop() {
            let attr: AttrVar = parse! { prop };
            assert_eq!(
                attr,
                AttrVar {
                    prop: true,
                    ..Default::default()
                }
            );
        }

        #[test]
        fn skip() {
            let attr: AttrVar = parse! { skip };
            assert_eq!(
                attr,
                AttrVar {
                    skip: true,
                    ..Default::default()
                }
            );
        }

        #[test]
        fn all() {
            let attr: AttrVar = parse! { name = "new", prop, skip };
            assert_eq!(
                attr,
                AttrVar {
                    name: Some("new".into()),
                    prop: true,
                    skip: true,
                }
            );
        }

        #[test]
        #[should_panic(expected = "Unknown field: `some`")]
        fn unknown_field() {
            let _attr: AttrVar = parse! { some = "other" };
        }

        #[test]
        #[should_panic(expected = "Unknown literal value `some` at prop")]
        fn unexpected_value() {
            let _attr: AttrVar = parse! { prop = "some" };
        }
    }

    mod fn_attrs {
        use super::*;

        #[test]
        fn empty() {
            let attr: AttrFn = parse! {};
            assert_eq!(attr, AttrFn::default());
        }

        #[test]
        fn name() {
            let attr: AttrFn = parse! { name = "new" };
            assert_eq!(
                attr,
                AttrFn {
                    name: Some("new".into()),
                    ..Default::default()
                }
            );
        }

        #[test]
        fn get() {
            let attr: AttrFn = parse! { get = "prop" };
            assert_eq!(
                attr,
                AttrFn {
                    get: Some("prop".into()),
                    ..Default::default()
                }
            );
        }

        #[test]
        fn set() {
            let attr: AttrFn = parse! { set = "prop" };
            assert_eq!(
                attr,
                AttrFn {
                    set: Some("prop".into()),
                    ..Default::default()
                }
            );
        }

        #[test]
        fn skip() {
            let attr: AttrFn = parse! { skip };
            assert_eq!(
                attr,
                AttrFn {
                    skip: true,
                    ..Default::default()
                }
            );
        }

        #[test]
        fn all() {
            let attr: AttrFn = parse! { name = "new", get = "prop", set = "prop", skip };
            assert_eq!(
                attr,
                AttrFn {
                    name: Some("new".into()),
                    get: Some("prop".into()),
                    set: Some("prop".into()),
                    skip: true,
                    ..Default::default()
                }
            );
        }

        #[test]
        #[should_panic(expected = "Unknown field: `some`")]
        fn unknown_field() {
            let _attr: AttrFn = parse! { some = "other" };
        }

        #[test]
        #[should_panic(expected = "Unexpected literal type `bool` at name")]
        fn unexpected_value() {
            let _attr: AttrFn = parse! { name = true };
        }
    }
}
