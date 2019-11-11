extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

#[proc_macro_derive(Builder)]
pub fn derive(tokens: TokenStream) -> TokenStream {
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
    let struct_name = input.ident;

    let generated_token = quote! {
        impl #struct_name {
            pub fn builder() {}
        }
    };

    TokenStream::from(generated_token)
}
