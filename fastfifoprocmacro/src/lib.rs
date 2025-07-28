#[cfg(test)]
use std::fs;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Error, Expr, GenericParam, Generics, Ident, Token, Type, TypeParamBound, Visibility, braced,
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    spanned::Spanned,
};

#[test]
fn test() {
    let ex_input = quote! {
        pub InOutUnion<Input, Output> {
            Producer: Input, atomic = false;
            Transformer: Output, atomic = true;
            Consumer: (), atomic = false;
        }
    };

    let ex_output = do_generate_union(parse_quote!(#ex_input));

    fs::write(
        std::path::Path::new("debug_output.rs"),
        ex_output.to_string(),
    )
    .unwrap();
}

/*
pub Name<generics> {
    VarientName: type, atomic = true;
    VarientName: type, atomic = false;
}
*/

struct UnionTypeInput {
    vis: Visibility,
    name: Ident,
    generics: Generics,
    variants: Vec<UnionVariant>,
}

impl Parse for UnionTypeInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let vis: Visibility = input.parse()?;
        let name: Ident = input.parse()?;
        let generics: Generics = input.parse()?;

        let content;
        braced!(content in input);

        let variants = content.parse_terminated(UnionVariant::parse, Token![;])?;

        let variants = variants.into_iter().collect::<Vec<_>>();

        Ok(Self {
            vis,
            name,
            generics,
            variants,
        })
    }
}

struct UnionVariant {
    name: Ident,
    atomicity: Expr,
    ty: Type,
}

impl Parse for UnionVariant {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;

        input.parse::<Token![:]>()?;

        let ty: Type = input.parse()?;

        input.parse::<Token![,]>()?;

        let atomicity = parse_atomic_expr(input)?;

        Ok(UnionVariant {
            name,
            ty,
            atomicity,
        })
    }
}

fn parse_atomic_expr(input: ParseStream) -> syn::Result<Expr> {
    let name: Ident = input.parse()?;

    if name != "atomic" {
        return Err(Error::new(name.span(), "expected `atomic`"));
    }

    input.parse::<Token![=]>()?;

    let expr: Expr = input.parse()?;

    expect_bool(&expr)?;

    Ok(expr)
}

#[derive(Clone)]
struct FullUnionVariant {
    variant_name: Ident,
    field_name: Ident,
    atomicity: Expr,
    ty: Type,
    chases: usize,
}

fn uv_to_fuv(uv: Vec<UnionVariant>) -> Vec<FullUnionVariant> {
    let n = uv.len();

    uv.into_iter()
        .enumerate()
        .map(
            |(
                i,
                UnionVariant {
                    name,
                    atomicity,
                    ty,
                },
            )| FullUnionVariant {
                variant_name: Ident::new(
                    stringcase::pascal_case(name.to_string().as_str()).as_str(),
                    name.span(),
                ),
                field_name: Ident::new(
                    stringcase::snake_case(name.to_string().as_str()).as_str(),
                    name.span(),
                ),
                atomicity,
                ty,
                chases: (i + n - 1) % n,
            },
        )
        .collect()
}

fn expect_bool(expr: &Expr) -> syn::Result<()> {
    match expr {
        Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Bool(_),
            ..
        }) => Ok(()),
        Expr::Binary(bin)
            if matches!(
                bin.op,
                syn::BinOp::Eq(_)
                    | syn::BinOp::Ne(_)
                    | syn::BinOp::Lt(_)
                    | syn::BinOp::Le(_)
                    | syn::BinOp::Gt(_)
                    | syn::BinOp::Ge(_)
                    | syn::BinOp::And(_)
                    | syn::BinOp::Or(_)
            ) =>
        {
            expect_bool(&*bin.left)?;
            expect_bool(&*bin.right)?;
            Ok(())
        }
        Expr::Unary(u) if matches!(u.op, syn::UnOp::Not(_)) => expect_bool(&*u.expr),
        _ => Err(syn::Error::new(
            expr.span(),
            "expected a boolean expression",
        )),
    }
}

fn unroll_variants(
    variants: Vec<FullUnionVariant>,
) -> (Vec<Ident>, Vec<Ident>, Vec<Expr>, Vec<Type>, Vec<usize>) {
    let mut vec1 = Vec::with_capacity(variants.len());
    let mut vec2 = Vec::with_capacity(variants.len());
    let mut vec3 = Vec::with_capacity(variants.len());
    let mut vec4 = Vec::with_capacity(variants.len());
    let mut vec5 = Vec::with_capacity(variants.len());

    for FullUnionVariant {
        variant_name,
        field_name,
        atomicity,
        ty,
        chases,
    } in variants
    {
        vec1.push(variant_name);
        vec2.push(field_name);
        vec3.push(atomicity);
        vec4.push(ty);
        vec5.push(chases);
    }

    (vec1, vec2, vec3, vec4, vec5)
}

fn get_chases<T: Clone>(chases: &Vec<usize>, original: &Vec<T>) -> Vec<T> {
    chases
        .iter()
        .map(|&i| original[i].clone())
        .collect::<Vec<_>>()
}

fn add_static_bound(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(type_param) = param {
            let has_static = type_param.bounds.iter().any(|b| {
                matches!(
                    b,
                    TypeParamBound::Lifetime(lt) if lt.ident == "static"
                )
            });
            if !has_static {
                type_param.bounds.push(parse_quote!('static));
            }
        }
    }
    generics
}

fn _is_unit(ty: &Type) -> bool {
    match ty {
        Type::Tuple(tuple) => tuple.elems.is_empty(),
        _ => false,
    }
}

pub(crate) fn do_generate_union(
    UnionTypeInput {
        vis,
        name,
        generics,
        variants,
    }: UnionTypeInput,
) -> proc_macro2::TokenStream {
    let num_variants = variants.len();

    let fifo_transform_path = quote! { ::fastfifo::transform };
    let fifo_config_path = quote! { #fifo_transform_path ::config };
    let entry_descriptor = quote! { #fifo_transform_path ::entry_descriptor::EntryDescriptor };
    let manually_drop = quote! { ::core::mem::ManuallyDrop };
    let result = quote! { #fifo_transform_path ::Result };

    let variants = uv_to_fuv(variants);

    let var_val_pair = variants
        .iter()
        .enumerate()
        .map(|(i, variant)| {
            let variant_name = variant.variant_name.clone();
            quote! { #variant_name = #i }
        })
        .collect::<Vec<_>>();

    let default_ty = &variants.last().unwrap().ty.clone();
    let default_field = &variants.last().unwrap().field_name.clone();

    let (variant_names, field_names, atomicities, types, chases) = unroll_variants(variants);

    let chases_variant_names = get_chases(&chases, &variant_names);
    let chases_field_names = get_chases(&chases, &field_names);
    let chases_types = get_chases(&chases, &types);
    // .into_iter()
    // .map(|ty| {
    //     if is_unit(&ty) {
    //         quote! {}
    //     } else {
    //         quote! { #ty }
    //     }
    // })
    // .collect::<Vec<_>>();

    let tag_name = format_ident!("{}Tag", name);
    let fifo_name = format_ident!("{}Fifo", name);
    let try_from_error_name = format_ident!("{}TryFromError", tag_name);

    let (variant_fifos, variant_entries) = variant_names
        .iter()
        .map(|variant| {
            let name_variant = format_ident!("{}{}", name, variant);
            (
                format_ident!("{}Fifo", name_variant),
                format_ident!("{}Entry", name_variant),
            )
        })
        .collect::<(Vec<_>, Vec<_>)>();

    let mut expanded_generics = generics.clone();

    let (impl_generic, ty_generic, where_clause) = generics.split_for_impl();

    expanded_generics
        .params
        .push(parse_quote!(const NUM_BLOCKS: usize));
    expanded_generics
        .params
        .push(parse_quote!(const BLOCK_SIZE: usize));
    // expanded_generics
    //     .params
    //     .push(parse_quote!(const NUM_TRANSFORMATIONS: usize));

    let expanded_where_clause = expanded_generics.make_where_clause();

    expanded_where_clause
        .predicates
        .push(parse_quote!([(); NUM_BLOCKS]:));
    expanded_where_clause
        .predicates
        .push(parse_quote!([(); BLOCK_SIZE]:));
    // expanded_where_clause
    //     .predicates
    //     .push(parse_quote!([(); NUM_TRANSFORMATIONS]:));

    let mut lifetime_generics = expanded_generics.clone();

    let static_expanded_generics = add_static_bound(expanded_generics.clone());

    let (static_expanded_impl_generic, _, _) = static_expanded_generics.split_for_impl();

    let (expanded_impl_generic, expanded_ty_generic, expanded_where_clause) =
        expanded_generics.split_for_impl();

    lifetime_generics
        .params
        .insert(0, parse_quote!('entry_descriptor_lifetime));

    let (lifetime_impl_generic, lifetime_ty_generic, _lifetime_where_clause) =
        lifetime_generics.split_for_impl();

    quote! {
        #vis union #name #impl_generic #where_clause {
            #( #field_names : #manually_drop <#types> ,)*
        }

        impl #impl_generic ::std::default::Default for #name #ty_generic #where_clause {
            fn default() -> Self {
                Self { #default_field : #manually_drop ::<#default_ty>::default() }
            }
        }

        impl #impl_generic ::core::convert::From<#default_ty> for #name #ty_generic #where_clause {
            fn from(val: #default_ty) -> Self {
                Self { #default_field: #manually_drop ::new(val) }
            }
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(usize)]
        #vis enum #tag_name {
            #( #var_val_pair ),*
        }

        impl ::core::convert::From<#tag_name> for usize {
            fn from(val: #tag_name) -> usize {
                val as usize
            }
        }

        #[derive(Debug)]
        #vis struct #try_from_error_name(usize);

        impl ::std::fmt::Display for #try_from_error_name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "attempted to turn {} into {}", self.0, stringify!(#tag_name))
            }
        }

        impl ::core::convert::TryFrom<usize> for #tag_name {
            type Error = #try_from_error_name;

            fn try_from(value: usize) -> ::std::result::Result<Self, Self::Error> {
                match value {
                    #( x if x == Self::#variant_names as usize => Ok(#tag_name::#variant_names) ,)*
                    x => Err(#try_from_error_name (x)),
                }
            }
        }

        impl #fifo_config_path ::FifoTag for #tag_name {
            fn is_atomic(self) -> bool {
                match self {
                    #( Self::#variant_names => #atomicities ,)*
                }
            }

            fn chases(self) -> Self {
                match self {
                    #( Self::#variant_names => Self::#chases_variant_names ,)*
                }
            }
        }

        impl #impl_generic #fifo_config_path ::IndexedDrop<#tag_name> for #name #ty_generic #where_clause {
            unsafe fn tagged_drop(&mut self, tag: #tag_name) {
                match tag {
                    #( #tag_name :: #variant_names => unsafe { #manually_drop ::drop(&mut self.#field_names) } ,)*
                }
            }
        }

        #vis struct #fifo_name #expanded_impl_generic (
            #fifo_transform_path ::FastFifo<#tag_name, #name #ty_generic, NUM_BLOCKS, BLOCK_SIZE, #num_variants>,
        ) #expanded_where_clause;

        impl #expanded_impl_generic #fifo_config_path ::TaggedClone<#tag_name> for #fifo_name #expanded_ty_generic #expanded_where_clause
        {
            fn unchecked_clone(&self) -> Self {
                Self(self.0.unchecked_clone())
            }
        }

        impl #static_expanded_impl_generic #fifo_name #expanded_ty_generic #expanded_where_clause {
            #[allow(dead_code)]
            pub fn new() -> Self {
                Self(#fifo_transform_path ::FastFifo::new())
            }

            #[allow(dead_code)]
            pub fn get_entry(&self, tag: #tag_name) -> #result <#entry_descriptor <'_, #tag_name, #name #ty_generic, BLOCK_SIZE, #num_variants>> {
                self.0.get_entry(tag)
            }

            #[allow(dead_code)]
            pub fn split(self) -> (
                #( #variant_fifos #expanded_ty_generic ,)*
            ) {
                (
                    #( #variant_fifos ( <Self as #fifo_config_path ::TaggedClone<#tag_name>>::unchecked_clone(&self)) ,)*
                )
            }
        }

        #(
            #vis struct #variant_entries #lifetime_impl_generic (
                #entry_descriptor <'entry_descriptor_lifetime, #tag_name, #name #ty_generic, BLOCK_SIZE, #num_variants>
            ) #expanded_where_clause;

            impl #lifetime_impl_generic From<#entry_descriptor <'entry_descriptor_lifetime, #tag_name, #name #ty_generic, BLOCK_SIZE, #num_variants>>
                for #variant_entries #lifetime_ty_generic #expanded_where_clause
            {
                fn from(value: #entry_descriptor <'entry_descriptor_lifetime, #tag_name, #name #ty_generic, BLOCK_SIZE, #num_variants>) -> Self {
                    Self(value)
                }
            }

            impl #lifetime_impl_generic Into<#entry_descriptor <'entry_descriptor_lifetime, #tag_name, #name #ty_generic, BLOCK_SIZE, #num_variants>>
                for #variant_entries #lifetime_ty_generic #expanded_where_clause
            {
                fn into(self) -> #entry_descriptor <'entry_descriptor_lifetime, #tag_name, #name #ty_generic, BLOCK_SIZE, #num_variants> {
                    self.0
                }
            }

            impl #lifetime_impl_generic #variant_entries #lifetime_ty_generic #expanded_where_clause {
                #[allow(dead_code)]
                pub fn transform<F: ::std::ops::FnOnce(#chases_types) -> #types>(&mut self, transformer: F) {
                    self.0.modify_t_in_place(|ptr| unsafe { ptr.write(
                        #name {
                            #field_names : #manually_drop ::new(
                                transformer(<#manually_drop ::<#chases_types>>::into_inner (ptr.read().#chases_field_names))
                            )
                        }
                    )})
                }
            }

            #vis struct #variant_fifos #expanded_impl_generic (
                #fifo_name #expanded_ty_generic
            ) #expanded_where_clause;

            impl #expanded_impl_generic #fifo_config_path ::TaggedClone<#tag_name> for #variant_fifos #expanded_ty_generic #expanded_where_clause {
                fn unchecked_clone(&self) -> Self {
                    Self(self.0.unchecked_clone())
                }
            }

            impl #expanded_impl_generic Clone for #variant_fifos #expanded_ty_generic #expanded_where_clause {
                fn clone(&self) -> Self {
                    <Self as #fifo_config_path ::TaggedClone<#tag_name>>::tagged_clone(&self, #tag_name :: #variant_names)
                        .expect("this variant was marked with `atomic = false` and cannot be cloned")
                }
            }

            impl #static_expanded_impl_generic #variant_fifos #expanded_ty_generic #expanded_where_clause {
                #[allow(dead_code)]
                pub fn get_entry<'entry_descriptor_lifetime>(&'entry_descriptor_lifetime self) -> #result <#variant_entries #lifetime_ty_generic> {
                    self.0.get_entry(#tag_name :: #variant_names).map(#variant_entries ::from)
                }

                #[allow(dead_code)]
                pub fn transform<F: ::std::ops::FnOnce(#chases_types) -> #types>(&self, transformer: F) -> #result <()> {
                    self.get_entry().map(|mut entry| entry.transform(transformer))
                }
            }
        )*
    }
}

#[proc_macro]
pub fn generate_union(input: TokenStream) -> TokenStream {
    do_generate_union(parse_macro_input!(input as UnionTypeInput)).into()
}
