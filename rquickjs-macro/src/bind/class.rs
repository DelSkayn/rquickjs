use super::{AttrData, AttrField, AttrImpl, BindConsts, BindFn, BindFns, BindProps, Binder};
use crate::{error, Config, Ident, Source, TokenStream};
use quote::{format_ident, quote};
use syn::{parse_quote, spanned::Spanned, Field, Fields, ItemEnum, ItemImpl, ItemStruct, Type};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BindClass {
    pub srcs: Vec<Source>,
    pub consts: BindConsts,
    pub props: BindProps,
    pub fns: BindFns,
    pub has_refs: bool,
}

impl BindClass {
    pub fn new(src: &Source, ident: &Ident) -> Self {
        Self {
            srcs: vec![src.with_ident(ident.clone())],
            ..Default::default()
        }
    }

    pub fn with_src(mut self, src: Source) -> Self {
        self.srcs.push(src);
        self
    }

    pub fn last_src(&self) -> &Source {
        let len = self.srcs.len();
        debug_assert!(len > 0);
        &self.srcs[len - 1]
    }

    pub fn first_src(&self) -> &Source {
        let len = self.srcs.len();
        debug_assert!(len > 0);
        &self.srcs[0]
    }

    pub fn expand(&self, name: &str, cfg: &Config) -> TokenStream {
        let lib_crate = &cfg.lib_crate;
        let exports_var = &cfg.exports_var;
        let src = self.first_src();

        let proto_list = self
            .props
            .iter()
            .filter(|(_, bind)| !bind.is_static())
            .map(|(name, bind)| bind.expand(name, cfg))
            .chain(
                self.fns
                    .iter()
                    .filter(|(_, bind)| bind.method)
                    .map(|(name, bind)| bind.expand(name, cfg)),
            )
            .collect::<Vec<_>>();

        let static_list = self
            .consts
            .iter()
            .map(|(name, bind)| bind.expand(name, cfg))
            .chain(
                self.props
                    .iter()
                    .filter(|(_, bind)| bind.is_static())
                    .map(|(name, bind)| bind.expand(name, cfg)),
            )
            .chain(
                self.fns
                    .iter()
                    .filter(|(name, func)| !func.method && (!func.constr || name.as_str() != "new"))
                    .map(|(name, bind)| bind.expand(name, cfg)),
            )
            .collect::<Vec<_>>();

        let ctor_func = self.fns.get("new").and_then(|func| {
            if func.constr {
                Some(func.expand(name, cfg))
            } else {
                None
            }
        });

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

                fn mark_refs(&self, marker: &RefsMarker) {
                    #lib_crate::HasRefs::mark_refs(self, marker);
                }
            })
        }

        quote! {
            impl #lib_crate::ClassDef for #src {
                const CLASS_NAME: &'static str = #name;

                fn class_id() -> &'static mut #lib_crate::ClassId {
                    static mut CLASS_ID: #lib_crate::ClassId = #lib_crate::ClassId::new();
                    unsafe { &mut CLASS_ID }
                }

                #(#extras)*
            }

            #lib_crate::Class::<#src>::register(_ctx)?;

            #ctor_func
        }
    }
}

impl Binder {
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
        } = self.get_attrs(attrs);

        if !self.visible(vis) || skip {
            return;
        }

        self.identify(&ident);

        let name = name.unwrap_or_else(|| ident.to_string());

        self.with_class(&name, &ident, |this| {
            if has_refs {
                this.top_class().unwrap().has_refs = true;
            }

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
        } = self.get_attrs(attrs);

        if !self.visible(vis) || skip {
            return;
        }

        self.identify(&ident);

        let name = name.unwrap_or_else(|| ident.to_string());

        self.with_class(&name, &ident, |this| {
            if has_refs {
                this.top_class().unwrap().has_refs = true;
            }
            // TODO support for variant fields
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
        let prop = self.top_prop(&name);
        let src = Source::default();

        prop.set_getter(span, &name, {
            let fn_ = format_ident!("get_{}", name);
            let self_ = format_ident!("self_");
            let def = parse_quote! {
                fn #fn_(#self_: #class) -> #ty {
                    #self_.#ident
                }
            };
            BindFn::new(&src, &fn_, &name, &[self_]).define(def)
        });

        if !readonly {
            prop.set_setter(span, &name, {
                let fn_ = format_ident!("set_{}", name);
                let self_ = format_ident!("self_");
                let val = format_ident!("val");
                let def = parse_quote! {
                    fn #fn_(#self_: #class, #val: #ty) {
                        #self_.#ident = #val;
                    }
                };
                BindFn::new(&src, &fn_, &name, &[self_, val]).define(def)
            });
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
        } = self.get_attrs(attrs);

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

        self.with_class(&name, &ident, |this| {
            if has_refs {
                this.top_class().unwrap().has_refs = true;
            }
            for item in items {
                this.bind_impl_item(item);
            }
        });
    }

    fn with_class<F>(&mut self, name: &str, ident: &Ident, func: F)
    where
        F: FnOnce(&mut Self),
    {
        let src = self.top_src();
        let decl = BindClass::new(src, ident);

        let decl = self
            .take_class(name)
            .map(|class| class.with_src(decl.last_src().clone()))
            .unwrap_or(decl);

        let decl = self.with(decl, func);

        self.top_classes().insert(name.into(), decl);
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

                fn class_id() -> &'static mut rquickjs::ClassId {
                    static mut CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new() ;
                    unsafe { &mut CLASS_ID }
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

                fn class_id() -> &'static mut rquickjs::ClassId {
                    static mut CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new() ;
                    unsafe { &mut CLASS_ID }
                }

                const HAS_PROTO: bool = true;

                fn init_proto<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Object<'js>) -> rquickjs::Result<()> {
                    exports.set("len", rquickjs::JsFn::new("len", rquickjs::Method(test::Node::len)))?;
                    exports.set("add", rquickjs::JsFn::new("add", rquickjs::Method(test::Node::add)))?;
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
                    #[quickjs(property, rename = "max_children")]
                    pub const MAX_CHILDREN: usize = 5;
                    // static prop
                    #[quickjs(getter = "root")]
                    pub fn get_root() -> &Node;
                    // class prop
                    #[quickjs(getter = "parent")]
                    pub fn get_parent(&self) -> Option<&Node>;
                    #[quickjs(setter = "parent")]
                    pub fn set_parent(&self, parent: &Node);
                }
            }
        } {
            impl rquickjs::ClassDef for test::Node {
                const CLASS_NAME: &'static str = "Node";

                fn class_id() -> &'static mut rquickjs::ClassId {
                    static mut CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new() ;
                    unsafe { &mut CLASS_ID }
                }

                const HAS_PROTO: bool = true;

                fn init_proto<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Object<'js>) -> rquickjs::Result<()> {
                    exports.prop("parent", (
                        rquickjs::JsFn::new("get_parent", rquickjs::Method(test::Node::get_parent)),
                        rquickjs::JsFn::new("set_parent", rquickjs::Method(test::Node::set_parent))
                    ))?;
                    Ok(())
                }

                const HAS_STATIC: bool = true;

                fn init_static<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Object<'js>) -> rquickjs::Result<()> {
                    exports.prop("max_children", (test::Node::MAX_CHILDREN,))?;
                    exports.prop("root", (rquickjs::JsFn::new("get_root", test::Node::get_root),))?;
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

                fn class_id() -> &'static mut rquickjs::ClassId {
                    static mut CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new() ;
                    unsafe { &mut CLASS_ID }
                }
            }
            rquickjs::Class::<test::Node>::register(_ctx)?;
            exports.set("Node", rquickjs::JsFn::new("new", rquickjs::Class::<test::Node>::constructor(test::Node::new)))?;
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

                fn class_id() -> &'static mut rquickjs::ClassId {
                    static mut CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new() ;
                    unsafe { &mut CLASS_ID }
                }

                const HAS_STATIC: bool = true;

                fn init_static<'js>(_ctx: rquickjs::Ctx<'js>, exports: &rquickjs::Object<'js>) -> rquickjs::Result<()> {
                    exports.set("TAG", test::Node::TAG)?;
                    exports.set("mix", rquickjs::JsFn::new("mix", test::Node::mix))?;
                    Ok(())
                }
            }
            rquickjs::Class::<test::Node>::register(_ctx)?;
            exports.set("Node", rquickjs::JsFn::new("new", rquickjs::Class::<test::Node>::constructor(test::Node::new)))?;
        };

        class_with_internal_refs { test } {
            #[quickjs(bare)]
            mod test {
                pub struct Node;
                impl Node {
                    pub fn mark_refs(&self, marker: &RefsMarker);
                }
            }
        } {
            impl rquickjs::ClassDef for test::Node {
                const CLASS_NAME: &'static str = "Node";

                fn class_id() -> &'static mut rquickjs::ClassId {
                    static mut CLASS_ID: rquickjs::ClassId = rquickjs::ClassId::new() ;
                    unsafe { &mut CLASS_ID }
                }

                const HAS_REFS: bool = true;

                fn mark_refs(&self, marker: &rquickjs::RefsMarker) {
                    Node::mark_refs(self, marker)
                }
            }
            rquickjs::Class::<test::Node>::register(_ctx)?;
        };
    }
}
