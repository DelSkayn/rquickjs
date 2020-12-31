use super::{AttrData, AttrField, AttrImpl, BindFn, BindFn1, BindItems, BindProp, Binder};
use crate::{Config, Ident, Source, TokenStream};
use quote::{format_ident, quote};
use syn::{parse_quote, spanned::Spanned, Field, Fields, ItemEnum, ItemImpl, ItemStruct, Type};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BindClass {
    pub src: Source,
    /// Static items
    pub items: BindItems,
    /// Prototype items
    pub proto_items: BindItems,
    /// Constructor
    pub ctor: Option<BindFn>,
    /// Has internal refs
    pub has_refs: bool,
}

impl BindClass {
    pub fn set_src(&mut self, ident: &Ident, name: &str, new_src: Source) {
        if self.src == Default::default() {
            self.src = new_src;
        } else if self.src != new_src {
            error!(
                ident,
                "Attempt to redefine class '{}' for `{}` which is already defined for `{}`",
                name,
                new_src,
                self.src
            );
        }
    }

    pub fn ctor(&mut self) -> &mut BindFn {
        if self.ctor.is_none() {
            self.ctor = Some(BindFn::default());
        }
        self.ctor.as_mut().unwrap()
    }

    pub fn expand(&self, name: &str, cfg: &Config) -> TokenStream {
        let lib_crate = &cfg.lib_crate;
        let exports_var = &cfg.exports_var;
        let src = &self.src;

        let proto_list = self
            .proto_items
            .iter()
            .map(|(name, bind)| bind.expand(name, cfg))
            .collect::<Vec<_>>();

        let static_list = self
            .items
            .iter()
            .map(|(name, bind)| bind.expand(name, cfg))
            .collect::<Vec<_>>();

        let ctor_func = self.ctor.as_ref().map(|func| func.expand(name, cfg));

        let mut extras = Vec::new();

        if !proto_list.is_empty() {
            extras.push(quote! {
                const HAS_PROTO: bool = true;

                fn init_proto<'js>(_ctx: #lib_crate::Ctx<'js>, #exports_var: &#lib_crate::Object<'js>) -> #lib_crate::Result<()> {
                    #(#proto_list)*
                    Ok(())
                }
            });
        }

        if !static_list.is_empty() {
            extras.push(quote! {
                const HAS_STATIC: bool = true;

                fn init_static<'js>(_ctx: #lib_crate::Ctx<'js>, #exports_var: &#lib_crate::Object<'js>) -> #lib_crate::Result<()> {
                    #(#static_list)*
                    Ok(())
                }
            });
        }

        if self.has_refs {
            extras.push(quote! {
                const HAS_REFS: bool = true;

                fn mark_refs(&self, marker: &#lib_crate::RefsMarker) {
                    #lib_crate::HasRefs::mark_refs(self, marker);
                }
            })
        }

        quote! {
            impl #lib_crate::ClassDef for #src {
                const CLASS_NAME: &'static str = #name;

                unsafe fn class_id() -> &'static #lib_crate::ClassId {
                    static CLASS_ID: #lib_crate::ClassId = #lib_crate::ClassId::new();
                    &CLASS_ID
                }

                #(#extras)*
            }

            #lib_crate::Class::<#src>::register(_ctx)?;

            #ctor_func
        }
    }
}

impl Binder {
    fn update_class(&mut self, ident: &Ident, name: &str, has_refs: bool) {
        let src = self.top_src().clone();
        let class = self.top_class().unwrap();
        class.set_src(ident, name, src);
        if has_refs {
            class.has_refs = true;
        }
    }

    pub(super) fn bind_struct(
        &mut self,
        ItemStruct {
            attrs,
            vis,
            ident,
            fields,
            ..
        }: &mut ItemStruct,
    ) {
        let AttrData {
            name,
            has_refs,
            skip,
            hide,
        } = self.get_attrs(attrs);

        self.hide_item(attrs, hide);

        if !self.visible(vis) || skip {
            return;
        }

        self.identify(&ident);

        let name = name.unwrap_or_else(|| ident.to_string());

        self.with_dir(ident, |this| {
            this.with_item::<BindClass, _>(&ident, &name, |this| {
                this.update_class(ident, &name, has_refs);

                use Fields::*;
                match fields {
                    Named(fields) => {
                        for field in fields.named.iter_mut() {
                            this.bind_field(None, field);
                        }
                    }
                    Unnamed(fields) => {
                        for (index, field) in fields.unnamed.iter_mut().enumerate() {
                            this.bind_field(Some(index), field)
                        }
                    }
                    _ => (),
                }
            });
        });
    }

    pub(super) fn bind_enum(
        &mut self,
        ItemEnum {
            attrs,
            vis,
            ident,
            //variants,
            ..
        }: &mut ItemEnum,
    ) {
        let AttrData {
            name,
            has_refs,
            skip,
            hide,
        } = self.get_attrs(attrs);

        self.hide_item(attrs, hide);

        if !self.visible(vis) || skip {
            return;
        }

        self.identify(&ident);

        let name = name.unwrap_or_else(|| ident.to_string());

        self.with_dir(ident, |this| {
            this.with_item::<BindClass, _>(&ident, &name, |this| {
                this.update_class(ident, &name, has_refs);

                // TODO support for variant fields
            });
        });
    }

    pub(super) fn bind_field(
        &mut self,
        index: Option<usize>,
        Field {
            attrs,
            vis,
            ident,
            ty,
            ..
        }: &mut Field,
    ) {
        let AttrField {
            name,
            readonly,
            skip,
        } = self.get_attrs(attrs);

        if !self.visible(vis) || skip {
            return;
        }

        let name = name
            .or_else(|| ident.as_ref().map(|ident| ident.to_string()))
            .or_else(|| index.map(|index| format!("{}", index)))
            .unwrap();

        let span = ident
            .as_ref()
            .map(|ident| ident.span())
            .unwrap_or_else(|| ty.span());

        let class = self.top_src().clone();
        if let Some(prop) = self.top_item::<BindProp, _>(span, &name, true) {
            let src = Source::default();

            prop.set_getter(span, &name, {
                let fn_ = format_ident!("get_{}", name);
                let self_ = format_ident!("self_");
                let def = parse_quote! {
                    fn #fn_(#self_: &#class) -> #ty {
                        #self_.#ident
                    }
                };
                let src = src.with_ident(fn_);
                BindFn1 {
                    src,
                    args: vec![self_],
                    method: true,
                    define: Some(def),
                    ..Default::default()
                }
            });

            if !readonly {
                prop.set_setter(span, &name, {
                    let fn_ = format_ident!("set_{}", name);
                    let self_ = format_ident!("self_");
                    let val = format_ident!("val");
                    let def = parse_quote! {
                        fn #fn_(#self_: &mut #class, #val: #ty) {
                            #self_.#ident = #val;
                        }
                    };
                    let src = src.with_ident(fn_);
                    BindFn1 {
                        src,
                        args: vec![self_, val],
                        method: true,
                        define: Some(def),
                        ..Default::default()
                    }
                });
            }
        }
    }

    pub(super) fn bind_impl(
        &mut self,
        ItemImpl {
            attrs,
            unsafety,
            trait_,
            self_ty,
            items,
            ..
        }: &mut ItemImpl,
    ) {
        let AttrImpl {
            name,
            has_refs,
            skip,
            hide,
        } = self.get_attrs(attrs);

        self.hide_item(attrs, hide);

        if let Some(unsafety) = unsafety {
            error!(
                unsafety.span,
                "Binding of unsafe impl blocks is weird and not supported."
            );
            return;
        }
        if let Some((_, path, _)) = trait_ {
            error!(path, "Binding of trait impls is weird and not supported.");
            return;
        }

        if skip {
            return;
        }

        let ident = if let Some(ident) = Self::class_ident(self_ty) {
            ident
        } else {
            return;
        };

        self.identify(&ident);

        let name = name.unwrap_or_else(|| ident.to_string());

        self.with_dir(ident, |this| {
            this.with_item::<BindClass, _>(&ident, &name, |this| {
                this.update_class(ident, &name, has_refs);

                this.bind_impl_items(items);
            });
        });
    }

    fn class_ident(ty: &Type) -> Option<&Ident> {
        if let Type::Path(path) = ty {
            if let Some(segment) = path.path.segments.last() {
                if segment.arguments.is_empty() {
                    return Some(&segment.ident);
                }
            }
        }
        error!(ty, "Parametrized types is not supported.");
        None
    }
}

#[cfg(test)]
mod test {
    test_cases! {
        simple_class { test } {
            #[quickjs(bare)]
            mod test {
                pub struct Test;
                impl Test {}
            }
        } {
            impl rquickjs::ClassDef for test::Test {
                const CLASS_NAME: &'static str = "Test";

                unsafe fn class_id() -> &'static rquickjs::ClassId {
                    static CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new() ;
                    &CLASS_ID
                }
            }
            rquickjs::Class::<test::Test>::register(_ctx)?;
        };

        class_with_fields { test } {
            #[quickjs(bare)]
            mod test {
                pub struct Test {
                    pub a: String,
                    pub b: f64,
                }
            }
        } {
            impl rquickjs::ClassDef for test::Test {
                const CLASS_NAME: &'static str = "Test";

                unsafe fn class_id() -> &'static rquickjs::ClassId {
                    static CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new() ;
                    &CLASS_ID
                }

                const HAS_PROTO: bool = true;

                fn init_proto<'js >(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Object<'js>) -> rquickjs::Result<()> {
                    exports.prop("a", rquickjs::Accessor::new({
                        fn get_a(self_: &test::Test) -> String {
                            self_.a
                        }
                        rquickjs::Method(get_a)
                    }, {
                        fn set_a(self_: &mut test::Test, val: String) {
                            self_.a = val;
                        }
                        rquickjs::Method(set_a)
                    }))?;
                    exports.prop("b", rquickjs::Accessor::new({
                        fn get_b(self_: &test::Test) -> f64 {
                            self_.b
                        }
                        rquickjs::Method(get_b)
                    }, {
                        fn set_b(self_: &mut test::Test, val: f64) {
                            self_.b = val;
                        }
                        rquickjs::Method(set_b)
                    }))?;
                    Ok (())
                }
            }
            rquickjs::Class::<test::Test>::register(_ctx)?;
        };

        class_with_methods { test } {
            #[quickjs(bare)]
            mod test {
                pub struct Node;
                impl Node {
                    pub fn len(&self) -> usize;
                    pub fn add(&self, child: &Node);
                }
            }
        } {
            impl rquickjs::ClassDef for test::Node {
                const CLASS_NAME: &'static str = "Node";

                unsafe fn class_id() -> &'static rquickjs::ClassId {
                    static CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new() ;
                    &CLASS_ID
                }

                const HAS_PROTO: bool = true;

                fn init_proto<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Object<'js>) -> rquickjs::Result<()> {
                    exports.set("len", rquickjs::Func::new("len", rquickjs::Method(test::Node::len)))?;
                    exports.set("add", rquickjs::Func::new("add", rquickjs::Method(test::Node::add)))?;
                    Ok(())
                }
            }
            rquickjs::Class::<test::Node>::register(_ctx)?;
        };

        class_with_props { test } {
            #[quickjs(bare)]
            mod test {
                pub struct Node;
                impl Node {
                    // static const prop
                    #[quickjs(value, rename = "children", proto)]
                    pub const HAS_CHILDREN: bool = true;
                    // const prop
                    #[quickjs(value, rename = "children", configurable)]
                    pub const MAX_CHILDREN: usize = 5;
                    // static prop
                    #[quickjs(rename = "root", get)]
                    pub fn get_root() -> &Node;
                    // instance prop
                    #[quickjs(get, enumerable)]
                    pub fn parent(&self) -> Option<&Node>;
                    #[quickjs(rename = "parent", set)]
                    pub fn set_parent(&self, parent: &Node);
                }
            }
        } {
            impl rquickjs::ClassDef for test::Node {
                const CLASS_NAME: &'static str = "Node";

                unsafe fn class_id() -> &'static rquickjs::ClassId {
                    static CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new() ;
                    &CLASS_ID
                }

                const HAS_PROTO: bool = true;

                fn init_proto<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Object<'js>) -> rquickjs::Result<()> {
                    exports.prop("children", rquickjs::Property::from(test::Node::HAS_CHILDREN))?;
                    exports.prop("parent", rquickjs::Accessor::new(
                        rquickjs::Method(test::Node::parent),
                        rquickjs::Method(test::Node::set_parent)
                    ).enumerable())?;
                    Ok(())
                }

                const HAS_STATIC: bool = true;

                fn init_static<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Object<'js>) -> rquickjs::Result<()> {
                    exports.prop("children", rquickjs::Property::from(test::Node::MAX_CHILDREN).configurable())?;
                    exports.prop("root", rquickjs::Accessor::new_get(test::Node::get_root))?;
                    Ok(())
                }
            }
            rquickjs::Class::<test::Node>::register(_ctx)?;
        };

        class_with_constructor { test } {
            #[quickjs(bare)]
            mod test {
                pub struct Node;
                impl Node {
                    pub fn new() -> Self;
                }
            }
        } {
            impl rquickjs::ClassDef for test::Node {
                const CLASS_NAME: &'static str = "Node";

                unsafe fn class_id() -> &'static rquickjs::ClassId {
                    static CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new() ;
                    &CLASS_ID
                }
            }
            rquickjs::Class::<test::Node>::register(_ctx)?;
            exports.set("Node", rquickjs::Func::new("Node", rquickjs::Class::<test::Node>::constructor(test::Node::new)))?;
        };

        class_with_static { test } {
            #[quickjs(bare)]
            mod test {
                pub struct Node;
                impl Node {
                    pub const TAG: i32 = 1;
                    pub fn new() -> Node {}
                    pub fn mix(a: &Node, b: &Node) {}
                }
            }
        } {
            impl rquickjs::ClassDef for test::Node {
                const CLASS_NAME: &'static str = "Node";

                unsafe fn class_id() -> &'static rquickjs::ClassId {
                    static CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new() ;
                    &CLASS_ID
                }

                const HAS_STATIC: bool = true;

                fn init_static<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Object<'js>) -> rquickjs::Result<()> {
                    exports.set("TAG", test::Node::TAG)?;
                    exports.set("mix", rquickjs::Func::new("mix", test::Node::mix))?;
                    Ok(())
                }
            }
            rquickjs::Class::<test::Node>::register(_ctx)?;
            exports.set("Node", rquickjs::Func::new("Node", rquickjs::Class::<test::Node>::constructor(test::Node::new)))?;
        };

        class_with_internal_refs { test } {
            #[quickjs(bare)]
            mod test {
                #[quickjs(has_refs)]
                pub struct Node;
            }
        } {
            impl rquickjs::ClassDef for test::Node {
                const CLASS_NAME: &'static str = "Node";

                unsafe fn class_id() -> &'static rquickjs::ClassId {
                    static CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new() ;
                    &CLASS_ID
                }

                const HAS_REFS: bool = true;

                fn mark_refs(&self, marker: &rquickjs::RefsMarker) {
                    rquickjs::HasRefs::mark_refs(self, marker);
                }
            }
            rquickjs::Class::<test::Node>::register(_ctx)?;
        };
    }
}
