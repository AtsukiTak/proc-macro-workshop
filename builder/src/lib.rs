extern crate proc_macro;

use proc_macro::TokenStream as StdTokenStream;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Builder)]
pub fn derive(tokens: StdTokenStream) -> StdTokenStream {
    let input = parse_macro_input!(tokens as DeriveInput);

    // ```
    // let struct_name = input.ident.to_string();
    // ```
    // とすると、 `proc-macro derive produced unparseable tokens`
    // というエラーが出る。
    // これはTokenStreamの導出には成功しているが、それがvalidな
    // ものではないことを意味している。
    // `quote` マクロは `ToToken` トレイトを実装している
    // あらゆる型を変数として受け入れるが、`ToToken` を
    // 実装していれば必ずvalidなTokenStreamを出力するとは限らない。
    // 例えば、`String` は `ToToken` をimplしているが、
    // それによって出力されるTokenは `stringリテラル` である。
    // そのため、`String` を型名が期待される位置にinterpolate
    // すると上記のようなエラーが出る。
    let struct_name = &input.ident;

    let builder_name = format_ident!("{}Builder", struct_name);

    let builder_fields = builder_fields(&input);

    let generated_token = quote! {
        pub struct #builder_name {
            #builder_fields
        }


        impl #builder_name {
            pub fn new() -> #builder_name {
                #builder_name {}
            }
        }

        impl #struct_name {
            pub fn builder() -> #builder_name {
                #builder_name::new()
            }
        }
    };

    StdTokenStream::from(generated_token)
}

fn builder_fields(input: &DeriveInput) -> TokenStream {
    let data = match input.data {
        syn::Data::Struct(ref data) => data,
        _ => panic!("Builder derive only supports struct"),
    };

    match data.fields {
        syn::Fields::Named(ref fields) => fields
            .named
            .iter()
            .cloned()
            .map(|field| {
                let name = field.ident.unwrap();
                let ty = field.ty;
                if field.colon_token.is_some() {
                    quote! {
                        #name : Option<#ty>,
                    }
                } else {
                    quote! {
                        #name : Option<#ty>
                    }
                }
            })
            .map(TokenStream::from)
            .collect(),
        _ => panic!("Builder derive only supports named fields"),
    }
}
