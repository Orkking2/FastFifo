#[cfg(test)]
use std::fs;

use itertools::izip;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Error, Expr, Generics, Ident, Token, Type, Visibility, braced,
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

fn is_unit(ty: &Type) -> bool {
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

    if num_variants < 2 {
        return quote! { compile_error!("Must have at least two layers!") };
    }

    let fifo_path = quote! { ::fastfifo };
    let fifo_config_path = quote! { #fifo_path ::config };
    let entry_descriptor = quote! { #fifo_path ::entry_descriptor::EntryDescriptor };
    let manually_drop = quote! { ::core::mem::ManuallyDrop };
    let result = quote! { #fifo_path ::Result };
    // let std_alloc = quote! { ::std::alloc };

    let (impl_generic, ty_generic, where_clause) = generics.split_for_impl();

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

    let producer_variant = variant_names.first().unwrap();

    let chases_variant_names = get_chases(&chases, &variant_names);
    let chases_field_names = get_chases(&chases, &field_names);
    let chases_types = get_chases(&chases, &types);

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

    let alloc_generics = generics.clone();

    // alloc_generics
    //     .params
    //     .push(parse_quote! { A: #std_alloc ::Allocator });

    let (alloc_impl_generic, alloc_ty_generic, _) = alloc_generics.split_for_impl();

    let default_alloc_generics = generics.clone();

    // default_alloc_generics
    //     .params
    //     .push(parse_quote! { A: #std_alloc ::Allocator = #std_alloc ::Global });

    let mut lifetime_generics = alloc_generics.clone();

    lifetime_generics
        .params
        .insert(0, parse_quote!('entry_descriptor_lifetime));

    let (lifetime_impl_generic, lifetime_ty_generic, _) = lifetime_generics.split_for_impl();

    let transform_f_trait = izip!(&chases_types, &types)
        .map(|(chases_type, ty)| {
            if is_unit(ty) && is_unit(chases_type) {
                quote! {::std::ops::FnOnce()}
            } else if is_unit(ty) {
                quote! {::std::ops::FnOnce(#chases_type)}
            } else if is_unit(chases_type) {
                quote! {::std::ops::FnOnce() -> #ty}
            } else {
                quote! {::std::ops::FnOnce(#chases_type) -> #ty}
            }
        })
        .collect::<Vec<_>>();

    let variant_impls = izip!(
        &variant_entries,
        &chases_types,
        &types,
        &field_names,
        &chases_field_names,
        &transform_f_trait
    )
    .map(|(variant_entry, chases_type, ty, field_name, chases_field_name, transform_trait)| {
        if is_unit(ty) && is_unit(chases_type) {
            quote! {
                impl #lifetime_impl_generic #variant_entry #lifetime_ty_generic #where_clause {
                    #[allow(dead_code)]
                    pub fn transform<F: #transform_trait>(&mut self, transformer: F) { transformer() }
                }
            }
        } else if is_unit(ty) {
            quote! {
                impl #lifetime_impl_generic #variant_entry #lifetime_ty_generic #where_clause {
                    #[allow(dead_code)]
                    pub fn transform<F: #transform_trait>(&mut self, transformer: F) {
                        self.0.modify_t_in_place(|ptr| unsafe {
                            transformer(<#manually_drop ::<#chases_type>>::into_inner (ptr.read().#chases_field_name))
                        })
                    }
                }
            }
        } else if is_unit(chases_type) {
            quote! {
                impl #lifetime_impl_generic #variant_entry #lifetime_ty_generic #where_clause {
                    #[allow(dead_code)]
                    pub fn transform<F: #transform_trait>(&mut self, transformer: F) {
                        self.0.modify_t_in_place(|ptr| unsafe { ptr.write(
                            #name {
                                #field_name : #manually_drop ::new(transformer())
                            }
                        )})
                    }
                }
            }
        } else {
            quote! {
                impl #lifetime_impl_generic #variant_entry #lifetime_ty_generic #where_clause {
                    #[allow(dead_code)]
                    pub fn transform<F: #transform_trait>(&mut self, transformer: F) {
                        self.0.modify_t_in_place(|ptr| unsafe { ptr.write(
                            #name {
                                #field_name : #manually_drop ::new(
                                    transformer(<#manually_drop ::<#chases_type>>::into_inner (ptr.read().#chases_field_name))
                                )
                            }
                        )})
                    }
                }
            }
        }
    }).collect::<Vec<_>>();

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

            fn producer() -> Self {
                Self::#producer_variant
            }

            fn num_transformations() -> usize {
                #num_variants
            }
        }

        impl #impl_generic #fifo_config_path ::IndexedDrop<#tag_name> for #name #ty_generic #where_clause {
            unsafe fn tagged_drop(&mut self, tag: #tag_name) {
                match tag {
                    #( #tag_name :: #variant_names => unsafe { #manually_drop ::drop(&mut self.#field_names) } ,)*
                }
            }
        }

        #vis struct #fifo_name #default_alloc_generics (
            #fifo_path ::FastFifo<#tag_name, #name #ty_generic>//, A>,
        ) #where_clause;

        impl #alloc_impl_generic #fifo_config_path ::TaggedClone<#tag_name> for #fifo_name #alloc_ty_generic #where_clause
        {
            fn unchecked_clone(&self) -> Self {
                Self(self.0.unchecked_clone())
            }
        }

        impl #impl_generic #fifo_name #ty_generic #where_clause {
            #[allow(dead_code)]
            pub fn new(num_blocks: usize, block_size: usize) -> Self {
                Self(#fifo_path ::FastFifo::new(num_blocks, block_size))
            }
        }

        impl #alloc_impl_generic #fifo_name #alloc_ty_generic #where_clause {
            // #[allow(dead_code)]
            // pub fn new_in(num_blocks: usize, block_size: usize, alloc: A) -> Self {
            //     Self(#fifo_path ::FastFifo::new_in(num_blocks, block_size, alloc))
            // }

            #[allow(dead_code)]
            pub fn get_entry(&self, tag: #tag_name) -> #result <#entry_descriptor <'_, #tag_name, #name #ty_generic>>{//, A>> {
                self.0.get_entry(tag)
            }

            #[allow(dead_code)]
            pub fn split(self) -> (
                #( #variant_fifos #alloc_ty_generic ,)*
            ) {
                (
                    #( #variant_fifos ( <Self as #fifo_config_path ::TaggedClone<#tag_name>>::unchecked_clone(&self)) ,)*
                )
            }
        }

        #(
            #vis struct #variant_entries #lifetime_impl_generic (
                #entry_descriptor <'entry_descriptor_lifetime, #tag_name, #name #ty_generic>//, A>
            ) #where_clause;

            impl #lifetime_impl_generic From<#entry_descriptor <'entry_descriptor_lifetime, #tag_name, #name #ty_generic>>//, A>>
                for #variant_entries #lifetime_ty_generic #where_clause
            {
                fn from(value: #entry_descriptor <'entry_descriptor_lifetime, #tag_name, #name #ty_generic>) -> Self {//, A>) -> Self {
                    Self(value)
                }
            }

            impl #lifetime_impl_generic Into<#entry_descriptor <'entry_descriptor_lifetime, #tag_name, #name #ty_generic>>//, A>>
                for #variant_entries #lifetime_ty_generic #where_clause
            {
                fn into(self) -> #entry_descriptor <'entry_descriptor_lifetime, #tag_name, #name #ty_generic>{//, A> {
                    self.0
                }
            }

            #variant_impls

            #vis struct #variant_fifos #alloc_impl_generic (
                #fifo_name #alloc_ty_generic
            ) #where_clause;

            impl #alloc_impl_generic #fifo_config_path ::TaggedClone<#tag_name> for #variant_fifos #alloc_ty_generic #where_clause {
                fn unchecked_clone(&self) -> Self {
                    Self(self.0.unchecked_clone())
                }
            }

            impl #alloc_impl_generic Clone for #variant_fifos #alloc_ty_generic #where_clause {
                fn clone(&self) -> Self {
                    <Self as #fifo_config_path ::TaggedClone<#tag_name>>::tagged_clone(&self, #tag_name :: #variant_names)
                        .expect("this variant was marked with `atomic = false` and cannot be cloned")
                }
            }

            impl #alloc_impl_generic #variant_fifos #alloc_ty_generic #where_clause {
                #[allow(dead_code)]
                pub fn get_entry<'entry_descriptor_lifetime>(&'entry_descriptor_lifetime self) -> #result <#variant_entries #lifetime_ty_generic> {
                    self.0.get_entry(#tag_name :: #variant_names).map(#variant_entries ::from)
                }

                #[allow(dead_code)]
                pub fn transform<F: #transform_f_trait>(&self, transformer: F) -> #result <()> {
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
