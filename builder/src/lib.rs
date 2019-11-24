extern crate proc_macro;

use proc_macro::TokenStream as StdTokenStream;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Builder)]
pub fn derive(tokens: StdTokenStream) -> StdTokenStream {
    let input = parse_macro_input!(tokens as DeriveInput);

    vec![
        ts_origin_impl_builder_fn(&input),
        ts_builder_struct(&input),
        ts_builder_impl_new_fn(&input),
        ts_builder_impl_fields_fn(&input),
        ts_builder_impl_build_fn(&input),
    ]
    .into_iter()
    .collect::<TokenStream>()
    .into()
}

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
fn origin_name(input: &DeriveInput) -> syn::Ident {
    input.ident.clone()
}

fn builder_name(input: &DeriveInput) -> syn::Ident {
    format_ident!("{}Builder", origin_name(input))
}

fn origin_fields<'a>(input: &'a DeriveInput) -> impl Iterator<Item = syn::Field> + 'a {
    let data = match input.data {
        syn::Data::Struct(ref data) => data,
        _ => panic!("Builder derive only supports struct"),
    };

    match data.fields {
        syn::Fields::Named(ref fields) => fields.named.iter().cloned(),
        _ => panic!("Builder derive only supports named fields"),
    }
}

fn ts_origin_impl_builder_fn(input: &DeriveInput) -> TokenStream {
    let origin_name = origin_name(input);
    let builder_name = builder_name(input);

    quote! {
        impl #origin_name {
            fn builder() -> #builder_name {
                #builder_name::new()
            }
        }
    }
}

/// This function returns `TokenStream` which represents
/// a code such as
/// ```ignore
/// struct CommandBuilder {
///     executable: Option<String>,
///     args: Option<Vec<String>>,
/// }
/// ```
fn ts_builder_struct(input: &DeriveInput) -> TokenStream {
    let builder_name = builder_name(input);
    let builder_fields: TokenStream = origin_fields(input)
        .map(|field| {
            let name = field.ident.unwrap();
            let ty = field.ty;
            quote! {
                #name : Option<#ty>,
            }
        })
        .collect();
    quote! {
        struct #builder_name {
            #builder_fields
        }
    }
}

///
/// This function returns `TokenStream` which represents
/// a code such as
/// ```ignore
/// impl CommandBuilder {
///     pub fn new() -> CommandBuilder {
///         CommandBuilder {
///             executable: None,
///             args: None,
///         }
///     }
/// }
/// ```
///
fn ts_builder_impl_new_fn(input: &DeriveInput) -> TokenStream {
    let builder_name = builder_name(input);
    let builder_initial_fields: TokenStream = origin_fields(input)
        .map(|field| {
            let name = field.ident.unwrap();
            quote! {
                #name: None,
            }
        })
        .collect();

    quote! {
        impl #builder_name {
            pub fn new() -> #builder_name {
                #builder_name {
                    #builder_initial_fields
                }
            }
        }
    }
}

///
/// This function returns `TokenStream` which represents
/// a code such as
/// ```ignore
/// impl CommandBuilder {
///     pub fn executable(&mut self, item: String) -> &mut Self {
///         self.executable = Some(item);
///         self
///     }
///
///     pub fn args(&mut self, item: Vec<String>) -> &mut Self {
///         self.args = Some(item);
///         self
///     }
/// }
/// ```
///
fn ts_builder_impl_fields_fn(input: &DeriveInput) -> TokenStream {
    let builder_name = builder_name(input);
    let builder_fn_fields: TokenStream = origin_fields(input)
        .map(|field| {
            let name = field.ident.unwrap();
            let ty = field.ty;
            quote! {
                pub fn #name(&mut self, item: #ty) -> &mut Self {
                    self.#name = Some(item);
                    self
                }
            }
        })
        .collect();

    quote! {
        impl #builder_name {
            #builder_fn_fields
        }
    }
}

/// This function produce TokenStream which represents
/// some source code such as
/// ```ignore
/// #[derive(Debug)]
/// pub struct BuildError();
///
/// impl CommandBuilder {
///     fn build(&mut self) -> Result<Command, BuildError> {
///         Ok(Command {
///             executable: self
///                 .executable
///                 .take()
///                 .ok_or(BuildError)?,
///             args: self
///                 .args
///                 .take()
///                 .ok_or(BuildError)?,
///         })
///     }
/// }
/// ```
fn ts_builder_impl_build_fn(input: &DeriveInput) -> TokenStream {
    let origin_name = origin_name(input);
    let builder_name = builder_name(input);
    let builder_fn_inner: TokenStream = origin_fields(input)
        .map(|field| {
            let name = field.ident.unwrap();
            quote! {
                #name: self.#name.take().ok_or(BuildError())?,
            }
        })
        .collect();

    quote! {
        #[derive(Debug)]
        pub struct BuildError();

        impl #builder_name {
            fn build(&mut self) -> Result<#origin_name, BuildError>
            {
                Ok(#origin_name {
                    #builder_fn_inner
                })
            }
        }
    }
}
