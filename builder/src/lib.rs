extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

#[proc_macro_derive(Builder)]
pub fn derive(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as DeriveInput);

    let data = match input.data {
        Data::Struct(d) => d,
        _ => panic!("Builder derive macro only supports struct."),
    };

    let fields = match data.fields {
        Fields::Named(f) => f,
        _ => panic!("Builder derive macro only supports named fields"),
    };

    let field_names = fields
        .named
        .iter()
        .map(|f| f.clone().ident.unwrap().to_string())
        .collect::<Vec<_>>();

    eprintln!("fields: {:#?}", field_names);

    TokenStream::new()
}
