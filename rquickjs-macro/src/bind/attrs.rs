use crate::{AttributeArgs, Config, Ident, Merge, Parenthesized, PubVis};
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
}

impl Merge for AttrVar {
    fn merge(&mut self, over: Self) {
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
}

impl Merge for AttrFn {
    fn merge(&mut self, over: Self) {
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
    /// Skip export
    pub skip: bool,
}

impl Merge for AttrData {
    fn merge(&mut self, over: Self) {
        if over.name.is_some() {
            self.name = over.name;
        }
        if over.has_refs {
            self.has_refs = true;
        }
        if over.skip {
            self.skip = true;
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

impl Merge for AttrField {
    fn merge(&mut self, over: Self) {
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

/// Impl attrs
#[derive(Default, FromMeta, Debug, PartialEq, Eq)]
#[darling(default)]
pub struct AttrImpl {
    /// Related class name
    #[darling(rename = "rename")]
    pub name: Option<String>,
    /// Data has internal refs
    pub has_refs: bool,
    /// Skip export
    pub skip: bool,
}

impl Merge for AttrImpl {
    fn merge(&mut self, over: Self) {
        if over.name.is_some() {
            self.name = over.name;
        }
        if over.has_refs {
            self.has_refs = true;
        }
        if over.skip {
            self.skip = true;
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

pub fn get_attrs<R: FromMeta + Default + Merge>(ident: &Ident, attrs: &mut Vec<Attribute>) -> R {
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
                    init: Some(Override::Inherit),
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
                    init: Some(Override::Explicit(quote::format_ident!(
                        "js_init_awesome_module"
                    ))),
                    ..Default::default()
                }
            );
        }

        #[test]
        fn public_default() {
            let attr: AttrItem = parse! { public };
            assert_eq!(
                attr,
                AttrItem {
                    public: Some(Override::Inherit),
                    ..Default::default()
                }
            );
        }

        #[test]
        fn public_restricted() {
            let attr: AttrItem = parse! { public = "crate" };
            assert_eq!(
                attr,
                AttrItem {
                    public: Some(Override::Explicit(PubVis::Crate)),
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
        fn rename() {
            let attr: AttrMod = parse! { rename = "new" };
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
            let attr: AttrMod = parse! { rename = "new", bare, skip };
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
        fn rename() {
            let attr: AttrVar = parse! { rename = "new" };
            assert_eq!(
                attr,
                AttrVar {
                    name: Some("new".into()),
                    ..Default::default()
                }
            );
        }

        #[test]
        fn property() {
            let attr: AttrVar = parse! { value };
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
            let attr: AttrVar = parse! { rename = "new", value, skip };
            assert_eq!(
                attr,
                AttrVar {
                    name: Some("new".into()),
                    prop: true,
                    skip: true,
                    ..Default::default()
                }
            );
        }

        #[test]
        #[should_panic(expected = "Unknown field: `some`")]
        fn unknown_field() {
            let _attr: AttrVar = parse! { some = "other" };
        }

        #[test]
        #[should_panic(expected = "Unknown literal value `some` at value")]
        fn unexpected_value() {
            let _attr: AttrVar = parse! { value = "some" };
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
        fn rename() {
            let attr: AttrFn = parse! { rename = "new" };
            assert_eq!(
                attr,
                AttrFn {
                    name: Some("new".into()),
                    ..Default::default()
                }
            );
        }

        #[test]
        fn getter() {
            let attr: AttrFn = parse! { get };
            assert_eq!(
                attr,
                AttrFn {
                    get: true,
                    ..Default::default()
                }
            );
        }

        #[test]
        fn setter() {
            let attr: AttrFn = parse! { set };
            assert_eq!(
                attr,
                AttrFn {
                    set: true,
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
            let attr: AttrFn = parse! { rename = "new", get, set, skip };
            assert_eq!(
                attr,
                AttrFn {
                    name: Some("new".into()),
                    get: true,
                    set: true,
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
        #[should_panic(expected = "Unexpected literal type `bool` at rename")]
        fn unexpected_value() {
            let _attr: AttrFn = parse! { rename = true };
        }
    }
}
