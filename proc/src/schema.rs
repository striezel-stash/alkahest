use proc_macro2::TokenStream;

fn field_param(idx: usize, ident: &Option<syn::Ident>) -> syn::Ident {
    match ident {
        Some(ident) => quote::format_ident!("__{}", ident),
        None => quote::format_ident!("__{}", idx),
    }
}

pub fn derive_schema(input: proc_macro::TokenStream) -> syn::Result<TokenStream> {
    let input = syn::parse::<syn::DeriveInput>(input)?;

    let data = input.data;

    let input = Input {
        vis: input.vis,
        ident: input.ident,
        generics: input.generics,
    };

    match data {
        syn::Data::Struct(data) => derive_schema_struct(input, data),
        syn::Data::Enum(data) => derive_schema_enum(input, data),
        syn::Data::Union(data) => Err(syn::Error::new_spanned(
            data.union_token,
            "Schema cannot be derived for unions",
        )),
    }
}

struct Input {
    vis: syn::Visibility,
    ident: syn::Ident,
    generics: syn::Generics,
}

fn derive_schema_struct(input: Input, data: syn::DataStruct) -> syn::Result<TokenStream> {
    let Input {
        vis,
        ident,
        generics,
    } = input;

    let has_fields = !data.fields.is_empty();

    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    let access_ident = quote::format_ident!("{}Access", ident);
    let serialize_ident = quote::format_ident!("{}Serialize", ident);

    let access_generics = syn::Generics {
        lt_token: generics
            .lt_token
            .or(has_fields.then(|| <syn::Token![<]>::default())),
        params: {
            if has_fields {
                std::iter::once(syn::parse_quote!('__a))
                    .chain(generics.params.iter().cloned())
                    .collect()
            } else {
                generics.params.clone()
            }
        },
        gt_token: generics
            .gt_token
            .or(has_fields.then(|| <syn::Token![>]>::default())),
        where_clause: generics.where_clause.clone(),
    };

    let (_access_impl_generics, access_type_generics, _access_where_clause) =
        access_generics.split_for_impl();

    let serialize_generics = syn::Generics {
        lt_token: has_fields.then(|| <syn::Token![<]>::default()),
        params: {
            data.fields
                .iter()
                .enumerate()
                .map(|(idx, field)| syn::GenericParam::Type(field_param(idx, &field.ident).into()))
                .collect()
        },
        gt_token: has_fields.then(|| <syn::Token![>]>::default()),
        where_clause: None,
    };

    let (_serialize_impl_generics, serialize_type_generics, _serialize_where_clause) =
        serialize_generics.split_for_impl();

    let mut impl_serialize_generics = serialize_generics.clone();
    impl_serialize_generics
        .params
        .extend(generics.params.iter().cloned());
    impl_serialize_generics.where_clause = Some({
        syn::WhereClause {
            where_token: <syn::Token![where]>::default(),
            predicates: {
                data.fields
                    .iter()
                    .enumerate()
                    .map(|(idx, field)| {
                        let ty = &field.ty;
                        syn::WherePredicate::Type(syn::PredicateType {
                            lifetimes: None,
                            bounded_ty: syn::Type::Path(syn::TypePath {
                                qself: None,
                                path: field_param(idx, &field.ident).into(),
                            }),
                            colon_token: <syn::Token![:]>::default(),
                            bounds: std::iter::once::<syn::TypeParamBound>(
                                syn::parse_quote!(::alkahest::Serialize<#ty>),
                            )
                            .collect(),
                        })
                    })
                    .collect()
            },
        }
    });

    let (impl_serialize_impl_generics, _impl_serialize_type_generics, impl_serialize_where_clause) =
        impl_serialize_generics.split_for_impl();

    let impl_serialize_header_arguments = match has_fields {
        false => syn::PathArguments::None,
        true => syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
            colon2_token: None,
            lt_token: <syn::Token![<]>::default(),
            args: {
                data.fields
                    .iter()
                    .enumerate()
                    .map(|(idx, field)| {
                        let p = field_param(idx, &field.ident);
                        let ty = &field.ty;
                        syn::GenericArgument::Type(
                            syn::parse_quote!{(<#p as ::alkahest::Serialize<#ty>>::Header, ::alkahest::private::usize)}
                        )
                    })
                    .collect()
            },
            gt_token: <syn::Token![>]>::default(),
        }),
    };

    let fields_ty = data
        .fields
        .iter()
        .map(|field| &field.ty)
        .collect::<Vec<_>>();

    let fields_param = data
        .fields
        .iter()
        .enumerate()
        .map(|(idx, field)| field_param(idx, &field.ident))
        .collect::<Vec<_>>();

    let fields_vis = data
        .fields
        .iter()
        .map(|field| &field.vis)
        .collect::<Vec<_>>();

    match &data.fields {
        syn::Fields::Named(_) => {
            let fields_ident = data
                .fields
                .iter()
                .map(|field| field.ident.as_ref().unwrap())
                .collect::<Vec<_>>();

            Ok(quote::quote! {
                #vis struct #access_ident #access_generics {
                    #(#fields_vis #fields_ident: <#fields_ty as ::alkahest::Schema>::Access<'__a>,)*
                }

                impl #impl_generics ::alkahest::Schema for #ident #type_generics #where_clause {
                    type Access<'__a> = #access_ident #access_type_generics;

                    #[inline]
                    fn header() -> ::alkahest::private::usize {
                        0 #(+ <#fields_ty as ::alkahest::Schema>::header())*
                    }

                    #[inline]
                    fn has_body() -> ::alkahest::private::bool {
                        false #(|| <#fields_ty as ::alkahest::Schema>::has_body())*
                    }

                    #[inline]
                    fn access<'__a>(input: &'__a [::alkahest::private::u8]) -> #access_ident #access_type_generics {
                        let mut offset = 0;
                        #access_ident {
                            #(#fields_ident: {
                                let cur = offset;
                                offset += <#fields_ty as ::alkahest::Schema>::header();
                                <#fields_ty as ::alkahest::Schema>::access(&input[cur..])
                            },)*
                        }
                    }
                }

                #[allow(non_camel_case_types)]
                #vis struct #serialize_ident #serialize_generics {
                    #(#fields_vis #fields_ident: #fields_param,)*
                }

                #[allow(non_camel_case_types)]
                impl #impl_serialize_impl_generics ::alkahest::Serialize<#ident #type_generics> for #serialize_ident #serialize_type_generics #impl_serialize_where_clause {
                    type Header = #serialize_ident #impl_serialize_header_arguments;

                    #[inline]
                    fn serialize_header(header: Self::Header, output: &mut [::alkahest::private::u8], offset: ::alkahest::private::usize) -> ::alkahest::private::bool {
                        let header_size = <#ident #type_generics as ::alkahest::Schema>::header();

                        if output.len() < header_size {
                            return false;
                        }

                        let mut total_offset = offset;
                        let mut output = output;
                        #(
                            let (field_header, field_offset) = header.#fields_ident;
                            let header_size = <#fields_ty as ::alkahest::Schema>::header();

                            let (head, tail) = output.split_at_mut(header_size);
                            output = tail;

                            <#fields_param as ::alkahest::Serialize<#fields_ty>>::serialize_header(field_header, head, total_offset + field_offset);
                            total_offset -= header_size;
                        )*

                        let _ = (output, total_offset);
                        true
                    }

                    #[inline]
                    fn serialize_body(self, output: &mut [::alkahest::private::u8]) -> ::alkahest::private::Result<(Self::Header, ::alkahest::private::usize), ::alkahest::private::usize> {
                        let mut headers_opt = #serialize_ident {
                            #(#fields_ident: None,)*
                        };

                        let mut written = 0;
                        let mut exhausted = false;
                        #(
                            let offset = written;
                            if !exhausted {
                                match <#fields_param as ::alkahest::Serialize<#fields_ty>>::serialize_body(self.#fields_ident, &mut output[offset..]) {
                                    Ok((header, size)) => {
                                        headers_opt.#fields_ident = Some((header, offset));
                                        written += size;
                                    }
                                    Err(size) => {
                                        exhausted = true;
                                        written += size;
                                    }
                                }
                            } else {
                                let size = <#fields_param as ::alkahest::Serialize<#fields_ty>>::body_size(self.#fields_ident);
                                written += size;
                            }
                        )*

                        if exhausted {
                            Err(written)
                        } else {
                            let header = #serialize_ident {
                                #(#fields_ident: headers_opt.#fields_ident.unwrap(),)*
                            };
                            Ok((header, written))
                        }
                    }
                }
            })
        }
        syn::Fields::Unnamed(_) => {
            let fileds_idx = (0..data.fields.len())
                .map(|idx| syn::Index::from(idx))
                .collect::<Vec<_>>();
            let field_nones = (0..data.fields.len()).map(|_| quote::format_ident!("None"));

            Ok(quote::quote! {
                #vis struct #access_ident #access_generics (
                    #(#fields_vis <#fields_ty as ::alkahest::Schema>::Access<'__a>,)*
                );

                impl #impl_generics ::alkahest::Schema for #ident #type_generics #where_clause {
                    type Access<'__a> = #access_ident #access_type_generics;

                    #[inline]
                    fn header() -> ::alkahest::private::usize {
                        0 #(+ <#fields_ty as ::alkahest::Schema>::header())*
                    }

                    #[inline]
                    fn has_body() -> ::alkahest::private::bool {
                        false #(|| <#fields_ty as ::alkahest::Schema>::has_body())*
                    }

                    #[inline]
                    fn access<'__a>(input: &'__a [::alkahest::private::u8]) -> #access_ident #access_type_generics {
                        let mut offset = 0;
                        #access_ident (
                            #({
                                let cur = offset;
                                offset += <#fields_ty as ::alkahest::Schema>::header();
                                <#fields_ty as ::alkahest::Schema>::access(&input[cur..])
                            },)*
                        )
                    }
                }

                #[allow(non_camel_case_types)]
                #vis struct #serialize_ident #serialize_generics (
                    #(#fields_vis #fields_param,)*
                );

                #[allow(non_camel_case_types)]
                impl #impl_serialize_impl_generics ::alkahest::Serialize<#ident #type_generics> for #serialize_ident #serialize_type_generics #impl_serialize_where_clause {
                    type Header = #serialize_ident #impl_serialize_header_arguments;

                    #[inline]
                    fn serialize_header(header: Self::Header, output: &mut [::alkahest::private::u8], offset: ::alkahest::private::usize) -> ::alkahest::private::bool {
                        let header_size = <#ident #type_generics as ::alkahest::Schema>::header();

                        if output.len() < header_size {
                            return false;
                        }

                        let mut total_offset = offset;
                        let mut output = output;
                        #(
                            let (field_header, field_offset) = header.#fileds_idx;
                            let header_size = <#fields_ty as ::alkahest::Schema>::header();

                            let (head, tail) = output.split_at_mut(header_size);
                            output = tail;

                            <#fields_param as ::alkahest::Serialize<#fields_ty>>::serialize_header(field_header, head, total_offset + field_offset);
                            total_offset -= header_size;
                        )*

                        let _ = (output, total_offset);
                        true
                    }

                    #[inline]
                    fn serialize_body(self, output: &mut [::alkahest::private::u8]) -> ::alkahest::private::Result<(Self::Header, ::alkahest::private::usize), ::alkahest::private::usize> {
                        let mut headers_opt = #serialize_ident(
                            #(#field_nones,)*
                        );

                        let mut written = 0;
                        let mut exhausted = false;
                        #(
                            let offset = written;
                            if !exhausted {
                                match <#fields_param as ::alkahest::Serialize<#fields_ty>>::serialize_body(self.#fileds_idx, &mut output[offset..]) {
                                    Ok((header, size)) => {
                                        headers_opt.#fileds_idx = Some((header, offset));
                                        written += size;
                                    }
                                    Err(size) => {
                                        exhausted = true;
                                        written += size;
                                    }
                                }
                            } else {
                                let size = <#fields_param as ::alkahest::Serialize<#fields_ty>>::body_size(self.#fileds_idx);
                                written += size;
                            }
                        )*

                        if exhausted {
                            Err(written)
                        } else {
                            let header = #serialize_ident(
                                #(headers_opt.#fileds_idx.unwrap(),)*
                            );
                            Ok((header, written))
                        }
                    }
                }
            })
        }
        syn::Fields::Unit => Ok(quote::quote! {
            #vis struct #access_ident #access_generics;

            impl #impl_generics ::alkahest::Schema for #ident #type_generics #where_clause {
                type Access<'__a> = #access_ident #access_type_generics;

                #[inline]
                fn header() -> ::alkahest::private::usize {
                    0
                }

                #[inline]
                fn has_body() -> ::alkahest::private::bool {
                    false
                }

                #[inline]
                fn access(_input: &[::alkahest::private::u8]) -> #access_ident #access_type_generics {
                    #access_ident
                }
            }

            #[allow(non_camel_case_types)]
            #vis struct #serialize_ident #serialize_generics;

            #[allow(non_camel_case_types)]
            impl #impl_serialize_impl_generics ::alkahest::Serialize<#ident #type_generics> for #serialize_ident #serialize_type_generics #impl_serialize_where_clause {
                type Header = #serialize_ident #impl_serialize_header_arguments;

                #[inline]
                fn serialize_header(_header: Self::Header, _output: &mut [::alkahest::private::u8], offset: ::alkahest::private::usize) -> ::alkahest::private::bool {
                    true
                }

                #[inline]
                fn serialize_body(self, _output: &mut [::alkahest::private::u8]) -> ::alkahest::private::Result<(Self::Header, ::alkahest::private::usize), ::alkahest::private::usize> {
                    let header = #serialize_ident;
                    Ok((header, 0))
                }
            }
        }),
    }
}

fn derive_schema_enum(input: Input, data: syn::DataEnum) -> syn::Result<TokenStream> {
    let Input {
        vis,
        ident,
        generics,
    } = input;

    let has_fields = data
        .variants
        .iter()
        .any(|variant| !variant.fields.is_empty());

    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    let access_ident = quote::format_ident!("{}Access", ident);

    let access_generics = syn::Generics {
        lt_token: generics
            .lt_token
            .or(has_fields.then(|| <syn::Token![<]>::default())),
        params: {
            if has_fields {
                std::iter::once(syn::parse_quote!('__a))
                    .chain(generics.params.iter().cloned())
                    .collect()
            } else {
                generics.params.clone()
            }
        },
        gt_token: generics
            .gt_token
            .or(has_fields.then(|| <syn::Token![>]>::default())),
        where_clause: generics.where_clause.clone(),
    };

    let (_access_impl_generics, access_type_generics, _access_where_clause) =
        access_generics.split_for_impl();

    let mut variants_access = Vec::new();
    let mut variants_header_size = Vec::new();
    let mut variants_has_body = Vec::new();
    let mut variants_access_construct = Vec::new();

    let mut result = quote::quote! {};
    let variants_idx = 0..data.variants.len() as u32;

    for (variant_idx, variant) in data.variants.into_iter().enumerate() {
        let variant_idx = variant_idx as u32;
        let variant_ident = &variant.ident;

        let serialize_ident = quote::format_ident!("{}{}Serialize", ident, variant_ident);

        let serialize_generics = syn::Generics {
            lt_token: (!variant.fields.is_empty()).then(|| <syn::Token![<]>::default()),
            params: {
                variant
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(idx, field)| {
                        syn::GenericParam::Type(field_param(idx, &field.ident).into())
                    })
                    .collect()
            },
            gt_token: (!variant.fields.is_empty()).then(|| <syn::Token![>]>::default()),
            where_clause: None,
        };

        let (_serialize_impl_generics, serialize_type_generics, _serialize_where_clause) =
            serialize_generics.split_for_impl();

        let mut impl_serialize_generics = serialize_generics.clone();
        impl_serialize_generics
            .params
            .extend(generics.params.iter().cloned());
        impl_serialize_generics.where_clause = Some({
            syn::WhereClause {
                where_token: <syn::Token![where]>::default(),
                predicates: {
                    variant
                        .fields
                        .iter()
                        .enumerate()
                        .map(|(idx, field)| {
                            let ty = &field.ty;
                            syn::WherePredicate::Type(syn::PredicateType {
                                lifetimes: None,
                                bounded_ty: syn::Type::Path(syn::TypePath {
                                    qself: None,
                                    path: field_param(idx, &field.ident).into(),
                                }),
                                colon_token: <syn::Token![:]>::default(),
                                bounds: std::iter::once::<syn::TypeParamBound>(
                                    syn::parse_quote!(::alkahest::Serialize<#ty>),
                                )
                                .collect(),
                            })
                        })
                        .collect()
                },
            }
        });

        let (
            impl_serialize_impl_generics,
            _impl_serialize_type_generics,
            impl_serialize_where_clause,
        ) = impl_serialize_generics.split_for_impl();

        let impl_serialize_header_arguments = match variant.fields.is_empty() {
            true => syn::PathArguments::None,
            false => syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                colon2_token: None,
                lt_token: <syn::Token![<]>::default(),
                args: {
                    variant.fields
                        .iter()
                        .enumerate()
                        .map(|(idx, field)| {
                            let p = field_param(idx, &field.ident);
                            let ty = &field.ty;
                            syn::GenericArgument::Type(
                                syn::parse_quote!{(<#p as ::alkahest::Serialize<#ty>>::Header, ::alkahest::private::usize)}
                            )
                        })
                        .collect()
                },
                gt_token: <syn::Token![>]>::default(),
            }),
        };

        let fields_ty = variant
            .fields
            .iter()
            .map(|field| &field.ty)
            .collect::<Vec<_>>();

        let fields_param = variant
            .fields
            .iter()
            .enumerate()
            .map(|(idx, field)| field_param(idx, &field.ident))
            .collect::<Vec<_>>();

        match &variant.fields {
            syn::Fields::Named(_) => {
                let fields_ident = variant
                    .fields
                    .iter()
                    .map(|field| field.ident.as_ref().unwrap())
                    .collect::<Vec<_>>();

                variants_access.push(quote::quote! {
                    #variant_ident{
                        #(#fields_ident: <#fields_ty as ::alkahest::Schema>::Access<'__a>,)*
                    }
                });

                variants_header_size
                    .push(quote::quote!({ 0 #(+ <#fields_ty as ::alkahest::Schema>::header())* }));

                variants_has_body.push(
                    quote::quote!({ false #(|| <#fields_ty as ::alkahest::Schema>::has_body())* }),
                );

                variants_access_construct.push(quote::quote! {
                    let mut offset = 0;
                    #access_ident::#variant_ident {
                        #(#fields_ident: {
                            let cur = offset;
                            offset += <#fields_ty as ::alkahest::Schema>::header();
                            <#fields_ty as ::alkahest::Schema>::access(&input[cur..])
                        },)*
                    }
                });

                let tokens = quote::quote! {
                    #[allow(non_camel_case_types)]
                    #vis struct #serialize_ident #serialize_generics {
                        #(#fields_ident: #fields_param,)*
                    }

                    #[allow(non_camel_case_types)]
                    impl #impl_serialize_impl_generics ::alkahest::Serialize<#ident #type_generics> for #serialize_ident #serialize_type_generics #impl_serialize_where_clause {
                        type Header = #serialize_ident #impl_serialize_header_arguments;

                        #[inline]
                        fn serialize_header(header: Self::Header, output: &mut [::alkahest::private::u8], offset: ::alkahest::private::usize) -> ::alkahest::private::bool {
                            let header_size = <#ident #type_generics as ::alkahest::Schema>::header();

                            if output.len() < header_size {
                                return false;
                            }

                            let (mut output, mut total_offset) = ::alkahest::private::write_variant_index(#variant_idx, output, offset);
                            #(
                                let (field_header, field_offset) = header.#fields_ident;
                                let header_size = <#fields_ty as ::alkahest::Schema>::header();

                                let (head, tail) = output.split_at_mut(header_size);
                                output = tail;

                                <#fields_param as ::alkahest::Serialize<#fields_ty>>::serialize_header(field_header, head, total_offset + field_offset);
                                total_offset -= header_size;
                            )*

                            let _ = (output, total_offset);
                            true
                        }

                        #[inline]
                        fn serialize_body(self, output: &mut [::alkahest::private::u8]) -> ::alkahest::private::Result<(Self::Header, ::alkahest::private::usize), ::alkahest::private::usize> {
                            let mut headers_opt = #serialize_ident {
                                #(#fields_ident: None,)*
                            };

                            let mut written = 0;
                            let mut exhausted = false;
                            #(
                                let offset = written;
                                if !exhausted {
                                    match <#fields_param as ::alkahest::Serialize<#fields_ty>>::serialize_body(self.#fields_ident, &mut output[offset..]) {
                                        Ok((header, size)) => {
                                            headers_opt.#fields_ident = Some((header, offset));
                                            written += size;
                                        }
                                        Err(size) => {
                                            exhausted = true;
                                            written += size;
                                        }
                                    }
                                } else {
                                    let size = <#fields_param as ::alkahest::Serialize<#fields_ty>>::body_size(self.#fields_ident);
                                    written += size;
                                }
                            )*

                            if exhausted {
                                Err(written)
                            } else {
                                let header = #serialize_ident {
                                    #(#fields_ident: headers_opt.#fields_ident.unwrap(),)*
                                };
                                Ok((header, written))
                            }
                        }
                    }
                };

                result.extend(tokens);
            }
            syn::Fields::Unnamed(_) => {
                variants_access.push(quote::quote! {
                    #variant_ident(
                        #(<#fields_ty as ::alkahest::Schema>::Access<'__a>,)*
                    )
                });

                variants_header_size
                    .push(quote::quote!({ 0 #(+ <#fields_ty as ::alkahest::Schema>::header())* }));

                variants_has_body.push(
                    quote::quote!({ false #(|| <#fields_ty as ::alkahest::Schema>::has_body())* }),
                );

                variants_access_construct.push(quote::quote! {
                    let mut offset = 0;
                    #access_ident::#variant_ident(
                        #({
                            let cur = offset;
                            offset += <#fields_ty as ::alkahest::Schema>::header();
                            <#fields_ty as ::alkahest::Schema>::access(&input[cur..])
                        },)*
                    )
                });

                let fileds_idx = (0..variant.fields.len())
                    .map(|idx| syn::Index::from(idx))
                    .collect::<Vec<_>>();
                let field_nones = (0..variant.fields.len()).map(|_| quote::format_ident!("None"));

                let tokens = quote::quote! {
                    #[allow(non_camel_case_types)]
                    #vis struct #serialize_ident #serialize_generics (
                        #(#fields_param,)*
                    );

                    #[allow(non_camel_case_types)]
                    impl #impl_serialize_impl_generics ::alkahest::Serialize<#ident #type_generics> for #serialize_ident #serialize_type_generics #impl_serialize_where_clause {
                        type Header = #serialize_ident #impl_serialize_header_arguments;

                        #[inline]
                        fn serialize_header(header: Self::Header, output: &mut [::alkahest::private::u8], offset: ::alkahest::private::usize) -> ::alkahest::private::bool {
                            let header_size = <#ident #type_generics as ::alkahest::Schema>::header();

                            if output.len() < header_size {
                                return false;
                            }

                            let (mut output, mut total_offset) = ::alkahest::private::write_variant_index(#variant_idx, output, offset);
                            #(
                                let (field_header, field_offset) = header.#fileds_idx;
                                let header_size = <#fields_ty as ::alkahest::Schema>::header();

                                let (head, tail) = output.split_at_mut(header_size);
                                output = tail;

                                <#fields_param as ::alkahest::Serialize<#fields_ty>>::serialize_header(field_header, head, total_offset + field_offset);
                                total_offset -= header_size;
                            )*

                            let _ = (output, total_offset);
                            true
                        }

                        #[inline]
                        fn serialize_body(self, output: &mut [::alkahest::private::u8]) -> ::alkahest::private::Result<(Self::Header, ::alkahest::private::usize), ::alkahest::private::usize> {
                            let mut headers_opt = #serialize_ident(
                                #(#field_nones,)*
                            );

                            let mut written = 0;
                            let mut exhausted = false;
                            #(
                                let offset = written;
                                if !exhausted {
                                    match <#fields_param as ::alkahest::Serialize<#fields_ty>>::serialize_body(self.#fileds_idx, &mut output[offset..]) {
                                        Ok((header, size)) => {
                                            headers_opt.#fileds_idx = Some((header, offset));
                                            written += size;
                                        }
                                        Err(size) => {
                                            exhausted = true;
                                            written += size;
                                        }
                                    }
                                } else {
                                    let size = <#fields_param as ::alkahest::Serialize<#fields_ty>>::body_size(self.#fileds_idx);
                                    written += size;
                                }
                            )*

                            if exhausted {
                                Err(written)
                            } else {
                                let header = #serialize_ident(
                                    #(headers_opt.#fileds_idx.unwrap(),)*
                                );
                                Ok((header, written))
                            }
                        }
                    }
                };

                result.extend(tokens);
            }
            syn::Fields::Unit => {
                variants_access.push(quote::quote! { #variant_ident });
                variants_header_size.push(quote::quote!({ 0 }));
                variants_has_body.push(quote::quote!({ false }));
                variants_access_construct.push(quote::quote! { #access_ident::#variant_ident });

                let tokens = quote::quote! {
                    #[allow(non_camel_case_types)]
                    #vis struct #serialize_ident #serialize_generics;

                    #[allow(non_camel_case_types)]
                    impl #impl_serialize_impl_generics ::alkahest::Serialize<#ident #type_generics> for #serialize_ident #serialize_type_generics #impl_serialize_where_clause {
                        type Header = #serialize_ident #impl_serialize_header_arguments;

                        #[inline]
                        fn serialize_header(_header: Self::Header, output: &mut [::alkahest::private::u8], offset: ::alkahest::private::usize) -> ::alkahest::private::bool {
                            let header_size = <#ident #type_generics as ::alkahest::Schema>::header();

                            if output.len() < header_size {
                                return false;
                            }

                            ::alkahest::private::write_variant_index(#variant_idx, output, offset);
                            true
                        }

                        #[inline]
                        fn serialize_body(self, _output: &mut [::alkahest::private::u8]) -> ::alkahest::private::Result<(Self::Header, ::alkahest::private::usize), ::alkahest::private::usize> {
                            let header = #serialize_ident;
                            Ok((header, 0))
                        }
                    }
                };

                result.extend(tokens);
            }
        }
    }

    result.extend(quote::quote! {
        #vis enum #access_ident #access_generics {
            #(#variants_access,)*
        }

        impl #impl_generics ::alkahest::Schema for #ident #type_generics #where_clause {
            type Access<'__a> = #access_ident #access_type_generics;

            #[inline]
            fn header() -> ::alkahest::private::usize {
                let mut max_header = 0;
                #(
                    if max_header < #variants_header_size {
                        max_header = #variants_header_size;
                    }
                )*

                max_header + ::alkahest::private::VARIANT_SIZE
            }

            #[inline]
            fn has_body() -> ::alkahest::private::bool {
                false #( || #variants_has_body )*
            }

            #[inline]
            fn access<'__a>(input: &'__a [::alkahest::private::u8]) -> #access_ident #access_type_generics {
                if input.len() < Self::header() {
                    ::alkahest::cold_panic!("input buffer is too small");
                }

                let (input, variant) = ::alkahest::private::read_variant_index(input);

                match variant {
                    #(#variants_idx => {
                        #variants_access_construct
                    })*

                    _ => {
                        ::alkahest::cold_panic!("invalid variant index")
                    }
                }
            }
        }
    });

    Ok(result)
}
