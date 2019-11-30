extern crate proc_macro;

use proc_macro::TokenStream as StdTokenStream;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive(tokens: StdTokenStream) -> StdTokenStream {
    let input = parse_macro_input!(tokens as DeriveInput);

    vec![
        ts_origin_impl_builder_fn(&input),
        ts_builder_struct(&input),
        ts_builder_impl_new_fn(&input),
        ts_builder_impl_fields_fn(&input),
        ts_builder_impl_each_field_fn(&input),
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

/// Returns `Type` of `T` in `Option<T>` or `Vec<T>` or something
/// like that.
/// Note that this function only be able to identify
/// if the type is written literally as `Option<T>`,
/// and not `std::option::Option<T>` or something like that.
fn single_generic_type_of(field: &syn::Field, type_name: &str) -> Option<syn::Type> {
    // the `std` in `std::option::Option`.
    let first_type_segment = match field.ty {
        syn::Type::Path(ref path) => path.path.segments.first().unwrap(),
        _ => return None,
    };
    if first_type_segment.ident == type_name {
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

fn is_path_eq(path: &syn::Path, expected: &str) -> bool {
    path.get_ident().map(|id| id == expected).unwrap_or(false)
}

/// Look for `#[builder(...)]` attribues and get the value and
/// return the `TokenStream` inside ().
fn get_builder_meta_items<'a>(field: &'a syn::Field) -> impl Iterator<Item = syn::NestedMeta> + 'a {
    field
        .attrs
        .iter()
        .filter(|attr| is_path_eq(&attr.path, "builder"))
        .flat_map(|attr| match attr.parse_meta() {
            Ok(syn::Meta::List(meta)) => meta.nested.into_iter(),
            _ => panic!("Unsupported attribute format"),
        })
}

/// Look for `#[builder(each = "...")]` attribute and get the
/// value of "...".
fn builder_attr_each(field: &syn::Field) -> Option<Result<syn::LitStr, syn::Error>> {
    get_builder_meta_items(field).find_map(|meta| match meta {
        syn::NestedMeta::Meta(syn::Meta::NameValue(syn::MetaNameValue {
            ref path,
            lit: syn::Lit::Str(ref s),
            ..
        })) => {
            if is_path_eq(path, "each") {
                Some(Ok(s.clone()))
            } else {
                Some(Err(syn::Error::new_spanned(
                    meta,
                    "expected `builder(each = \"...\")`",
                )))
            }
        }
        _ => None,
    })
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
///     // multiple value field
///     args: Vec<String>,
///     // optional field
///     current_dir: Option<String>,
/// }
/// ```
fn ts_builder_struct(input: &DeriveInput) -> TokenStream {
    let builder_name = builder_name(input);
    let builder_fields: TokenStream = origin_fields(input)
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            if let Some(ty) = single_generic_type_of(&field, "Option") {
                quote! {
                    #name: Option<#ty>,
                }
            } else if let Some(ty) = single_generic_type_of(&field, "Vec") {
                quote! {
                    #name: Vec<#ty>,
                }
            } else {
                let ty = field.ty;
                quote! {
                    #name : Option<#ty>,
                }
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
            let name = field.ident.as_ref().unwrap();
            if single_generic_type_of(&field, "Vec").is_some() {
                quote! {
                    #name: Vec::new(),
                }
            } else {
                quote! {
                    #name: None,
                }
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
fn ts_builder_impl_fields_fn(input: &DeriveInput) -> TokenStream {
    let builder_name = builder_name(input);
    let builder_fn_fields: TokenStream = origin_fields(input)
        .filter(|field| {
            // #[builder(each = "...")] の値と同じ場合はスキップする
            match builder_attr_each(field) {
                Some(Ok(ref s)) => *field.ident.as_ref().unwrap() != s.value(),
                _ => true,
            }
        })
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            // `T` when field type is `Option<T>` or `T`.
            if single_generic_type_of(&field, "Vec").is_some() {
                let ty = field.ty;
                quote! {
                    pub fn #name(&mut self, item: #ty) -> &mut Self {
                        self.#name = item;
                        self
                    }
                }
            } else {
                let ty = single_generic_type_of(&field, "Option").unwrap_or(field.ty);
                quote! {
                    pub fn #name(&mut self, item: #ty) -> &mut Self {
                        self.#name = Some(item);
                        self
                    }
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

/// This function returns `TokenStream` which represents
/// a code such as
/// ```ignore
/// impl CommandBuilder {
///     pub fn args(&mut self, item: String) -> &mut Self {
///         self.args.push(item);
///         self
///     }
/// }
/// ```
fn ts_builder_impl_each_field_fn(input: &DeriveInput) -> TokenStream {
    let builder_name = builder_name(input);
    let builder_funcs: TokenStream = origin_fields(input)
        .filter_map(|field| match builder_attr_each(&field) {
            Some(Err(e)) => Some(e.to_compile_error()),
            Some(Ok(each_fn_name_str)) => {
                let each_fn_name = syn::Ident::new(
                    each_fn_name_str.value().as_ref(),
                    proc_macro2::Span::call_site(),
                );
                let name = field.ident.as_ref().unwrap();
                let ty = single_generic_type_of(&field, "Vec").expect(
                    "#[builder(each = \"...\")] attribute is only able to be set on `Vec` type",
                );

                let ts = quote! {
                    pub fn #each_fn_name(&mut self, item: #ty) -> &mut Self {
                        self.#name.push(item);
                        self
                    }
                };
                Some(ts)
            }
            None => None,
        })
        .collect();

    quote! {
        impl #builder_name {
            #builder_funcs
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
            if single_generic_type_of(&field, "Option").is_some() {
                // optional field
                quote! {
                    #name: self.#name.take(),
                }
            } else if single_generic_type_of(&field, "Vec").is_some() {
                quote! {
                    #name: std::mem::replace(&mut self.#name, Vec::new()),
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
