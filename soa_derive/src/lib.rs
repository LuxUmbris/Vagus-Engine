//! Copyright © 2026 Vagus Engine Contributors
//! The authors of this file (the "Software") are not reliable
//! for any damages caused by the Software.
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//! NEITHER EXPRESS OR IMPLIED. EVERY COPY OF THE SOFTWARE MUST
//! CONTAIN THIS TEXT. ANY MODIFICATIONS OF ANY KIND OF THE
//! SOFTWARE MUST BE EXPLICITLY STATED.
//!
//! For further information, see:
//! github.com/LuxUmbris/Vagus-Engine/LICENSE.
//!
//! Authors: Vincent Noll

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, parse_macro_input, parse_quote};

#[proc_macro_derive(Soa)]
pub fn derive_soa(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let storage_name = format_ident!("{}Storage", name);
    let generics = &input.generics;
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    let fields = match &input.data {
        Data::Struct(data) => &data.fields,
        _ => {
            return syn::Error::new_spanned(name, "Soa can only be derived for structs")
                .to_compile_error()
                .into();
        }
    };

    let (field_names, field_types, field_access): (Vec<_>, Vec<_>, Vec<_>) = match fields {
        Fields::Named(fields) => (
            fields
                .named
                .iter()
                .map(|field| field.ident.clone().expect("named field"))
                .collect(),
            fields.named.iter().map(|field| field.ty.clone()).collect(),
            fields
                .named
                .iter()
                .map(|field| {
                    let name = field.ident.clone().expect("named field");
                    quote!(#name)
                })
                .collect(),
        ),
        Fields::Unnamed(fields) => (
            (0..fields.unnamed.len())
                .map(|i| format_ident!("_{i}"))
                .collect(),
            fields
                .unnamed
                .iter()
                .map(|field| field.ty.clone())
                .collect(),
            (0..fields.unnamed.len())
                .map(|i| {
                    let index = syn::Index::from(i);
                    quote!(#index)
                })
                .collect(),
        ),
        Fields::Unit => (Vec::new(), Vec::new(), Vec::new()),
    };
    let storage_row_bytes = field_types.iter().fold(quote!(0usize), |sum, field_type| {
        quote! { #sum + ::core::mem::size_of::<#field_type>() }
    });

    let value_names: Vec<_> = (0..field_names.len())
        .map(|i| format_ident!("__soa_value_{i}"))
        .collect();
    let row_from_values = match fields {
        Fields::Named(_) => quote!(Self { #(#field_names: #value_names,)* }),
        Fields::Unnamed(_) => quote!(Self(#(#value_names,)*)),
        Fields::Unit => quote!(Self),
    };
    let clone_row = match fields {
        Fields::Named(_) => quote! {
            Self { #(#field_names: storage.#field_names.get(index)?.clone(),)* }
        },
        Fields::Unnamed(_) => quote! {
            Self(#(storage.#field_names.get(index)?.clone(),)*)
        },
        Fields::Unit => quote!(Self),
    };
    let row_exists = if field_names.is_empty() {
        quote!(true)
    } else {
        quote!(#(storage.#field_names.get(index).is_some())&&*)
    };
    let all_columns_nonempty = if field_names.is_empty() {
        quote!(true)
    } else {
        quote!(#(storage.#field_names.len() > 0)&&*)
    };
    let all_columns_long_enough = if field_names.is_empty() {
        quote!(true)
    } else {
        quote!(#(storage.#field_names.len() >= len)&&*)
    };
    let all_columns_exact_len = if field_names.is_empty() {
        quote!(true)
    } else {
        quote!(#(storage.#field_names.len() == len)&&*)
    };
    let all_other_columns_exact_len = if field_names.is_empty() {
        quote!(true)
    } else {
        quote!(#(other.#field_names.len() == additional)&&*)
    };

    let mut clone_generics = input.generics.clone();
    if !field_types.is_empty() {
        let where_clause = clone_generics.make_where_clause();
        for field_type in &field_types {
            where_clause
                .predicates
                .push(parse_quote!(#field_type: ::core::clone::Clone));
        }
    }
    let (clone_impl_generics, clone_type_generics, clone_where_clause) =
        clone_generics.split_for_impl();

    let has_no_columns = field_names.is_empty();
    let has_columns = !has_no_columns;
    let storage_state = if has_no_columns {
        quote! { __soa_len: usize, __soa_capacity: usize, }
    } else {
        quote! {}
    };
    let storage_state_init = if has_no_columns {
        quote! { __soa_len: 0, __soa_capacity: capacity, }
    } else {
        quote! {}
    };
    let prepare_append_body = if has_no_columns {
        quote! {
            if additional > storage.__soa_capacity.saturating_sub(storage.__soa_len) {
                return Err(::std::io::Error::new(
                    ::std::io::ErrorKind::OutOfMemory,
                    "SoA capacity exceeded",
                ));
            }
            Ok(())
        }
    } else {
        quote! {
            #(
                storage.#field_names.prepare_additional_relocating(additional)?;
            )*
            Ok(())
        }
    };
    let append_storage_body = if has_no_columns {
        quote! {
            if other.__soa_len != additional {
                return Err(::std::io::Error::new(
                    ::std::io::ErrorKind::InvalidData,
                    "SoA source row count is inconsistent",
                ));
            }
            Self::prepare_append(storage, additional)?;
            storage.__soa_len += additional;
            other.__soa_len = 0;
            other.__soa_capacity = other_capacity;
            Ok(())
        }
    } else {
        quote! {
            if !(#all_other_columns_exact_len) {
                return Err(::std::io::Error::new(
                    ::std::io::ErrorKind::InvalidData,
                    "SoA source columns have inconsistent lengths",
                ));
            }
            let replacement = Self::create_storage(other_capacity)?;
            Self::prepare_append(storage, additional)?;
            #(
                storage.#field_names.reserve_append(&other.#field_names)?;
            )*
            #(
                storage.#field_names.append(&mut other.#field_names);
            )*
            *other = replacement;
            Ok(())
        }
    };
    let grow_storage_body = if has_no_columns {
        quote! {
            if storage.__soa_len != len {
                return Err(::std::io::Error::new(
                    ::std::io::ErrorKind::InvalidData,
                    "SoA row count is inconsistent",
                ));
            }
            if capacity < len {
                return Err(::std::io::Error::new(
                    ::std::io::ErrorKind::InvalidInput,
                    "SoA capacity is smaller than its row count",
                ));
            }
            let mut grown = Self::create_storage(capacity)?;
            grown.__soa_len = storage.__soa_len;
            storage.__soa_len = 0;
            storage.__soa_capacity = 0;
            Ok(grown)
        }
    } else {
        quote! {
            if !(#all_columns_exact_len) {
                return Err(::std::io::Error::new(
                    ::std::io::ErrorKind::InvalidData,
                    "SoA columns have inconsistent lengths",
                ));
            }
            if capacity < len {
                return Err(::std::io::Error::new(
                    ::std::io::ErrorKind::InvalidInput,
                    "SoA capacity is smaller than its row count",
                ));
            }
            #(
                storage.#field_names.grow_to(capacity)?;
            )*
            let mut grown = Self::create_storage(0)?;
            #(
                grown.#field_names.reserve_append(&storage.#field_names)?;
            )*
            #(
                grown.#field_names.append(&mut storage.#field_names);
            )*
            Ok(grown)
        }
    };
    let push_to_body = if has_no_columns {
        quote! {
            let _ = value;
            if storage.__soa_len == storage.__soa_capacity {
                return Err(::std::io::Error::new(
                    ::std::io::ErrorKind::OutOfMemory,
                    "SoA capacity exceeded",
                ));
            }
            storage.__soa_len += 1;
            Ok(())
        }
    } else {
        quote! {
            #(
                storage.#field_names.prepare_push()?;
            )*
            #(
                storage.#field_names.push(value.#field_access)?;
            )*
            Ok(())
        }
    };
    let insert_body = if has_no_columns {
        quote! {
            let _ = value;
            if index > storage.__soa_len {
                return Err(::std::io::Error::new(
                    ::std::io::ErrorKind::InvalidInput,
                    "SoA insertion index out of bounds",
                ));
            }
            if storage.__soa_len == storage.__soa_capacity {
                return Err(::std::io::Error::new(
                    ::std::io::ErrorKind::OutOfMemory,
                    "SoA capacity exceeded",
                ));
            }
            storage.__soa_len += 1;
            Ok(())
        }
    } else {
        quote! {
            #(
                storage.#field_names.prepare_push()?;
            )*
            #(
                storage.#field_names.insert(index, value.#field_access)?;
            )*
            Ok(())
        }
    };
    let replace_at_body = if has_no_columns {
        quote! {
            if index >= storage.__soa_len {
                return Err(value);
            }
            Ok(value)
        }
    } else {
        quote! {
            if !(#row_exists) {
                return Err(value);
            }
            #(
                let #value_names = ::core::mem::replace(
                    storage.#field_names.get_mut(index).expect("checked SoA row"),
                    value.#field_access,
                );
            )*
            Ok(#row_from_values)
        }
    };
    let remove_body = if has_no_columns {
        quote! {
            if index >= storage.__soa_len {
                return None;
            }
            storage.__soa_len -= 1;
            Some(Self)
        }
    } else {
        quote! {
            if !(#row_exists) {
                return None;
            }
            #(
                let #value_names = storage.#field_names.remove(index)?;
            )*
            Some(#row_from_values)
        }
    };
    let swap_remove_body = if has_no_columns {
        quote! {
            if index >= storage.__soa_len {
                return None;
            }
            storage.__soa_len -= 1;
            Some(Self)
        }
    } else {
        quote! {
            if !(#row_exists) {
                return None;
            }
            #(
                let #value_names = storage.#field_names.swap_remove(index)?;
            )*
            Some(#row_from_values)
        }
    };
    let pop_body = if has_no_columns {
        quote! {
            if storage.__soa_len == 0 {
                return None;
            }
            storage.__soa_len -= 1;
            Some(Self)
        }
    } else {
        quote! {
            if !(#all_columns_nonempty) {
                return None;
            }
            #(
                let #value_names = storage.#field_names.pop()?;
            )*
            Some(#row_from_values)
        }
    };
    let pop_front_body = if has_no_columns {
        quote! {
            if storage.__soa_len == 0 {
                return None;
            }
            storage.__soa_len -= 1;
            Some(Self)
        }
    } else {
        quote! {
            if !(#all_columns_nonempty) {
                return None;
            }
            #(
                let #value_names = storage.#field_names.pop_front()?;
            )*
            Some(#row_from_values)
        }
    };
    let truncate_body = if has_no_columns {
        quote! {
            if len > storage.__soa_len {
                return false;
            }
            storage.__soa_len = len;
            true
        }
    } else {
        quote! {
            if !(#all_columns_long_enough) {
                return false;
            }
            #(
                storage.#field_names.truncate(len);
            )*
            true
        }
    };
    let clear_body = if has_no_columns {
        quote! { storage.__soa_len = 0; }
    } else {
        quote! {
            #(
                storage.#field_names.clear();
            )*
        }
    };
    let clone_row = if has_no_columns {
        quote! {
            if index < storage.__soa_len {
                Some(Self)
            } else {
                None
            }
        }
    } else {
        quote! { Some(#clone_row) }
    };

    quote! {
        #[derive(Clone, Debug, PartialEq)]
        pub struct #storage_name #generics {
            #(pub #field_names: ::vagus::essential::collections::StableVec<#field_types>,)*
            #storage_state
        }

        impl #impl_generics ::vagus::ecs::Soa for #name #type_generics #where_clause {
            type Storage = #storage_name #type_generics;
            const HAS_COLUMNS: bool = #has_columns;
            const STORAGE_ROW_BYTES: Option<usize> = Some(#storage_row_bytes);

            fn create_storage(capacity: usize) -> ::std::io::Result<Self::Storage> {
                Ok(#storage_name {
                    #(
                        #field_names: ::vagus::essential::collections::StableVec::with_capacity(capacity)
                            .ok_or_else(|| ::std::io::Error::other("failed to reserve SoA column"))?,
                    )*
                    #storage_state_init
                })
            }

            fn grow_storage(
                storage: &mut Self::Storage,
                len: usize,
                capacity: usize,
            ) -> ::std::io::Result<Self::Storage> {
                #grow_storage_body
            }

            fn prepare_append(
                storage: &mut Self::Storage,
                additional: usize,
            ) -> ::std::io::Result<()> {
                #prepare_append_body
            }

            fn append_storage(
                storage: &mut Self::Storage,
                other: &mut Self::Storage,
                additional: usize,
                other_capacity: usize,
            ) -> ::std::io::Result<()> {
                #append_storage_body
            }

            fn push_to(
                storage: &mut Self::Storage,
                value: Self,
            ) -> ::std::io::Result<()> {
                #push_to_body
            }

            fn insert(
                storage: &mut Self::Storage,
                index: usize,
                value: Self,
            ) -> ::std::io::Result<()> {
                #insert_body
            }

            fn replace_at(
                storage: &mut Self::Storage,
                index: usize,
                value: Self,
            ) -> Result<Self, Self> {
                #replace_at_body
            }

            fn remove(storage: &mut Self::Storage, index: usize) -> Option<Self> {
                #remove_body
            }

            fn swap_remove(storage: &mut Self::Storage, index: usize) -> Option<Self> {
                #swap_remove_body
            }

            fn pop(storage: &mut Self::Storage) -> Option<Self> {
                #pop_body
            }

            fn pop_front(storage: &mut Self::Storage) -> Option<Self> {
                #pop_front_body
            }

            fn truncate(storage: &mut Self::Storage, len: usize) -> bool {
                #truncate_body
            }

            fn clear(storage: &mut Self::Storage) {
                #clear_body
            }
        }

        impl #clone_impl_generics ::vagus::ecs::SoaClone
            for #name #clone_type_generics #clone_where_clause
        {
            fn clone_at(storage: &Self::Storage, index: usize) -> Option<Self> {
                #clone_row
            }
        }
    }
    .into()
}
