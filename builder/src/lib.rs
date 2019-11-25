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

/// Returns `AngleBracketedGenericArguments` which represents
/// the `T` in `Option<T>` if given field is
/// `Option<T>`.
/// Note that this function only be able to identify
/// if the type is written literally as `Option<T>` and not
/// `std::option::Option<T>` or something like that.
fn generics_of_option_type(field: &syn::Field) -> Option<syn::Type> {
    // the `std` in `std::option::Option`.
    let first_type_segment = match field.ty {
        syn::Type::Path(ref path) => path.path.segments.first().unwrap(),
        _ => return None,
    };
    if first_type_segment.ident == "Option" {
        let generic_arg = match first_type_segment.arguments {
            syn::PathArguments::AngleBracketed(ref args) => args.args.first().unwrap(),
            _ => unreachable!(),
        };
        match generic_arg {
            syn::GenericArgument::Type(ref ty) => Some(ty.clone()),
            _ => unreachable!(),
        }
    } else {
        return None;
    }
}

/// This function returns `TokenStream` which represents
/// a code such as
/// ```ignore
/// impl Command {
///     fn builder() -> CommandBuilder {
///         CommandBuilder::new()
///     }
/// }
/// ```
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
///
/// ```ignore
/// struct CommandBuilder {
///     executable: Option<String>,
///     // optional field
///     current_dir: Option<String>,
/// }
/// ```
///
/// Note that original `Command` struct is such as
///
/// ```ignore
/// struct Command {
///     executable: String,
///     // optional field
///     current_dir: Option<String>,
/// }
/// ```
fn ts_builder_struct(input: &DeriveInput) -> TokenStream {
    let builder_name = builder_name(input);
    let builder_fields: TokenStream = origin_fields(input)
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            // `T` in `Option<T>` or just `T`.
            let ty = generics_of_option_type(&field).unwrap_or(field.ty);
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
///             current_dir: None,
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
///     pub fn current_dir(&mut self, item: String) -> &mut Self {
///         self.current_dir = Some(item);
///         self
///     }
/// }
/// ```
///
fn ts_builder_impl_fields_fn(input: &DeriveInput) -> TokenStream {
    let builder_name = builder_name(input);
    let builder_fn_fields: TokenStream = origin_fields(input)
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            // `T` when field type is `Option<T>` or `T`.
            let ty = generics_of_option_type(&field).unwrap_or(field.ty);
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
///             // `current_dir` is optional field
///             current_dir: self
///                 .current_dir
///                 .take(),
///         })
///     }
/// }
/// ```
fn ts_builder_impl_build_fn(input: &DeriveInput) -> TokenStream {
    let origin_name = origin_name(input);
    let builder_name = builder_name(input);
    let builder_fn_inner: TokenStream = origin_fields(input)
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            if generics_of_option_type(&field).is_some() {
                // optional field
                quote! {
                    #name: self.#name.take(),
                }
            } else {
                // required field
                quote! {
                    #name: self.#name.take().ok_or(BuildError())?,
                }
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
