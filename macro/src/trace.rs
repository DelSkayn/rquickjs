use proc_macro2::TokenStream;
use proc_macro_error::abort;
use quote::quote;
use syn::{Data, DataStruct, DeriveInput};

use crate::{class::add_js_lifetime, crate_ident, fields::Field};

pub(crate) fn expand(input: DeriveInput) -> TokenStream {
    let DeriveInput {
        ident,
        generics,
        data,
        ..
    } = input;

    let struct_ = match data {
        Data::Struct(x) => x,
        Data::Enum(e) => {
            abort!(
                e.enum_token,
                "implementing trace for enums is not yet supported"
            )
        }
        Data::Union(u) => {
            abort!(u.union_token, "deriving trace for unions is not supported");
        }
    };

    let DataStruct { fields, .. } = struct_;

    let lifetime_generics = add_js_lifetime(&generics);
    let lib_crate = crate_ident();

    let parsed_fields = Field::parse_fields(&fields);
    let trace_impls = parsed_fields
        .iter()
        .enumerate()
        .map(|(idx, f)| f.expand_trace_body(&lib_crate, idx));

    quote! {
        impl #lifetime_generics #lib_crate::class::Trace<'js> for #ident #generics{
            fn trace<'a>(&self, _tracer: #lib_crate::class::Tracer<'a,'js>){
                #(#trace_impls)*
            }
        }
    }
}
