use proc_macro2::{Ident, TokenStream, Span};
use quote::{quote, quote_spanned, ToTokens};
use syn::{
    parse::Nothing, parse_macro_input, spanned::Spanned, Data, DataEnum, DataStruct, DeriveInput,
    Error, ItemFn, Lit, Meta, NestedMeta, Pat, PatIdent, PatType,
};

// RUSTFLAGS="-Z macro-backtrace" cargo test

fn handle_item_fn(node: &ItemFn) -> syn::Result<TokenStream> {
    let return_type = match node.sig.output {
        syn::ReturnType::Default => {
            return Err(Error::new(
                node.sig.output.span(),
                r#"unexpected return type"#,
            ))
        }
        syn::ReturnType::Type(_, ref path) => path.to_token_stream(),
    };

    let actix_macro = node
        .attrs
        .iter()
        .find(|attr| attr.path.get_ident().map(Ident::to_string) == Some("post".to_string()));

    let actix_macro = if let Some(actix_macro) = actix_macro {
        actix_macro
    } else {
        return Err(Error::new(
            Span::call_site(),
            r#"could not find actix `post` attribute macro"#,
        ));
    };
    let url_path = actix_macro.parse_meta()?;
    let url_path = match url_path {
        Meta::List(meta_list) => match meta_list.nested.iter().next() {
            Some(NestedMeta::Lit(Lit::Str(str))) => str.value(),
            None => return Err(Error::new(meta_list.span(), r#"expected a literal string"#)),
            err => return Err(Error::new(err.span(), r#"expected a literal string"#)),
        },
        err => return Err(Error::new(err.span(), r#"expected a list"#)),
    };

    let arg_type = node
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Receiver(_) => None,
            syn::FnArg::Typed(PatType { pat, ty, .. }) => {
                if let Pat::Ident(PatIdent { ident, .. }) = &**pat {
                    if *ident == "input" || *ident == "_input" {
                        return Some(ty.to_token_stream());
                    }
                }
                None
            }
        })
        .next();

    if let Some(arg_type) = arg_type {
        let name = &node.sig.ident;
        let name_string = node.sig.ident.to_string();

        let typescriptable_arg_type_name = quote_spanned! {arg_type.span()=>
            <#arg_type as tucant::typescript::Typescriptable>::name()
        };

        let typescriptable_arg_type_code = quote_spanned! {arg_type.span()=>
            <#arg_type as tucant::typescript::Typescriptable>::code()
        };

        let typescriptable_return_type_name = quote_spanned! {return_type.span()=>
            <#return_type as tucant::typescript::Typescriptable>::name()
        };

        let typescriptable_return_type_code = quote_spanned! {return_type.span()=>
            <#return_type as tucant::typescript::Typescriptable>::code()
        };

        Ok(quote! {
            #node

            impl tucant::typescript::Typescriptable for #name {
                fn name() -> String {
                    #name_string.to_string()
                }

                fn code() -> ::std::collections::BTreeSet<String> {
                    let mut result = ::std::collections::BTreeSet::from(["export async function ".to_string() + &<#name as tucant::typescript::Typescriptable>::name() + "(input: " + &#typescriptable_arg_type_name + ")"
                    + ": Promise<" + &#typescriptable_return_type_name + "> {" +
                    r#"
    return await genericFetch("http://localhost:8080"# + #url_path + r#"", input) as "# + &#typescriptable_return_type_name +
        "\n}"]);
                    result.extend(#typescriptable_arg_type_code);
                    result.extend(#typescriptable_return_type_code);
                    result
                }
            }
        })
    } else {
        Err(Error::new(
            node.sig.inputs.span(),
            r#"name one of the parameters `input` or `_input`"#,
        ))
    }
}

fn typescriptable_impl(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let name_string = input.ident.to_string();

    let (members, members_code) = match &input.data {
        Data::Struct(DataStruct { fields, .. }) => match fields {
            syn::Fields::Named(fields_named) => {
                let (members, members_code) = fields_named
                .named
                .iter()
                .map(|field| {
                    let ident_string = field.ident.as_ref().unwrap().to_string();

                    let ts_type_attr = field.attrs.iter().find(|attr| {
                        attr.path.get_ident().map(Ident::to_string) == Some("ts_type".to_string())
                    });

                    let serde_attr = field.attrs.iter().find(|attr| {
                        attr.path.get_ident().map(Ident::to_string) == Some("serde".to_string())
                    });

                    let field_type = if let Some(ts_type_attr) = ts_type_attr {
                        match ts_type_attr.parse_meta()? {
                            Meta::List(meta_list) => match meta_list.nested.iter().next() {
                                Some(NestedMeta::Meta(Meta::Path(path))) => path.to_token_stream(),
                                None => return Err(Error::new(meta_list.span(), r#"expected a type"#)),
                                _ => return Err(Error::new(meta_list.span(), r#"expected a type"#)),
                            },
                            err => return Err(Error::new(err.span(), r#"expected a list"#)),
                        }
                    } else {
                        if let Some(serde_attr) = serde_attr {
                            let serde_attr = serde_attr.parse_meta()?;
                            match serde_attr {
                                Meta::List(meta) => {
                                    if let Some(meta) = meta.nested.iter().find(|meta| {
                                        match meta {
                                            NestedMeta::Meta(Meta::NameValue(meta)) => {
                                                meta.path.get_ident().map(Ident::to_string) == Some("serialize_with".to_string())
                                                || meta.path.get_ident().map(Ident::to_string) == Some("deserialize_with".to_string())
                                            },
                                            _ => false,
                                        }
                                    }) {
                                        return Err(Error::new(meta.span(), r#"`serde` attribute macro `serialize_with` or `deserialize_with` requires `ts_type` attribute macro to clarify type"#))
                                    }
                                }
                                _ => {}
                            }
                        }

                        field.ty.to_token_stream()
                    };

                    let typescriptable_field_type_name = quote_spanned! {field_type.span()=>
                        <#field_type as tucant::typescript::Typescriptable>::name()
                    };

                    let typescriptable_field_type_code = quote_spanned! {field_type.span()=>
                        <#field_type as tucant::typescript::Typescriptable>::code()
                    };

                    Ok((
                        quote! {
                           "  " + #ident_string + ": " + &#typescriptable_field_type_name + ",\n"
                        },
                        quote! {
                            result.extend(#typescriptable_field_type_code);
                        },
                    ))
                })
                .collect::<Result<Vec<_>, _>>()?
                .iter()
                .fold((quote! {}, quote! {}), |(accx, accy), (x, y)| {
                    (
                        quote! {
                            #accx + #x
                        },
                        quote! {
                            #accy
                            #y
                        },
                    )
                });
                (quote! {
                    + "{\n"
                    #members
                    + "}"
                }, members_code)
            },
            err => {
                return Err(Error::new(
                    err.span(),
                    r#"only structs with named fields supported"#,
                ))
            }
        },
        Data::Enum(DataEnum { variants, .. }) => variants
            .iter()
            .map(|variant| match &variant.fields {
                syn::Fields::Named(err) => Err(Error::new(
                    err.span(),
                    r#"only enums with unnamed fields allowed"#,
                )),
                syn::Fields::Unnamed(fields) => {
                    let mut iter = fields.unnamed.iter();
                    if let Some(field) = iter.next() {
                        if let Some(field) = iter.next() {
                            Err(Error::new(
                                field.span(),
                                r#"exactly one field in enum allowed"#,
                            ))
                        } else {
                            Ok((variant, field))
                        }
                    } else {
                        Err(Error::new(
                            variant.fields.span(),
                            r#"exactly one field in enum allowed"#,
                        ))
                    }
                }
                syn::Fields::Unit => Err(Error::new(
                    variant.fields.span(),
                    r#"only enums with unnamed fields allowed"#,
                )),
            })
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .map(|(variant, field)| {
                let ident_string = variant.ident.to_string();
                let field_type = &field.ty;

                let typescriptable_field_type_name = quote_spanned! {field_type.span()=>
                    <#field_type as tucant::typescript::Typescriptable>::name()
                };

                let typescriptable_field_type_code = quote_spanned! {field_type.span()=>
                    <#field_type as tucant::typescript::Typescriptable>::code()
                };

                (
                    quote! {
                       "{ type: \"" + #ident_string + "\", value: " + &#typescriptable_field_type_name + " }\n"
                    },
                    quote! {
                        result.extend(#typescriptable_field_type_code);
                    },
                )
            })
            .fold((quote! {}, quote! {}), |(accx, accy), (x, y)| {
                (
                    quote! {
                        #accx + " | " + #x
                    },
                    quote! {
                        #accy
                        #y
                    },
                )
            }),
        Data::Union(err) => {
            return Err(Error::new(
                err.union_token.span(),
                r#"unions not supported"#,
            ))
        }
    };

    Ok(quote! {
        impl tucant::typescript::Typescriptable for #name {
            fn name() -> String {
                #name_string.to_string()
            }

            fn code() -> ::std::collections::BTreeSet<String> {
                let mut result = ::std::collections::BTreeSet::from(["export type ".to_string() + &#name::name() + " =\n"
                #members
                ]);
                #members_code
                result
            }
        }
    })
}

#[proc_macro_attribute]
pub fn ts(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    parse_macro_input!(attr as Nothing);
    let input = parse_macro_input!(item as ItemFn);

    proc_macro::TokenStream::from(handle_item_fn(&input).unwrap_or_else(Error::into_compile_error))
}

#[proc_macro_derive(Typescriptable, attributes(ts_type))]
pub fn typescriptable(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    proc_macro::TokenStream::from(
        typescriptable_impl(input).unwrap_or_else(Error::into_compile_error),
    )
}

#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {}
}
