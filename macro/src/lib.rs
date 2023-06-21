use proc_macro::TokenStream as TokenStream1;
use proc_macro_error::{abort, proc_macro_error};
use syn::{parse_macro_input, Item};

mod class;

#[proc_macro_attribute]
#[proc_macro_error]
pub fn jsclass(_attr: TokenStream1, item: TokenStream1) -> TokenStream1 {
    let item = parse_macro_input!(item as Item);
    match item {
        Item::Struct(item) => TokenStream1::from(class::derive(item)),
        item => {
            abort!(item, "#[jsclass] macro can only be used on structs")
        }
    }
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn jsfunction(attr: TokenStream1, item: TokenStream1) -> TokenStream1 {
    let item = parse_macro_input!(item as Item);
    match item {
        Item::Fn(_) => todo!(),
        item => {
            abort!(item, "#[jsfunction] macro can only be used on functions")
        }
    }
}
