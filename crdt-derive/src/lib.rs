extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DataStruct, DeriveInput, Fields, parse_macro_input};

/// The entry point for the `Crdt` derive procedural macro.
///
/// This macro provides an automatic implementation of the `Crdt` trait for structs.
/// It implements the `merge` method by calling `merge` on each field of the struct
/// individually. This effectively treats the struct as a "Product CRDT".
#[proc_macro_derive(Crdt)]
pub fn derive_crdt(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match generate_crdt_impl(input) {
        Ok(token_stream) => token_stream.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Orchestrates the generation of the `Crdt` trait implementation.
///
/// This function validates that the input is a supported data type (structs only)
/// and constructs the final implementation block.
fn generate_crdt_impl(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let generics = &input.generics;

    let merge_body = match &input.data {
        Data::Struct(data_struct) => generate_merge_body(data_struct),
        Data::Enum(_) => {
            return Err(syn::Error::new(
                name.span(),
                "Derive(Crdt) is currently only supported for structs. Enums require a custom merge strategy.",
            ));
        }
        Data::Union(_) => {
            return Err(syn::Error::new(
                name.span(),
                "Derive(Crdt) is not supported for unions.",
            ));
        }
    };

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics crdt::Crdt for #name #ty_generics #where_clause {
            type Value = Self;

            fn merge(&mut self, other: &Self) {
                #merge_body
            }

            fn value(&self) -> Self::Value {
                self.clone()
            }
        }
    })
}

/// Generates the code for the `merge` method's body based on the struct fields.
///
/// It handles three types of structs:
/// 1. Named structs (e.g., `struct Foo { a: T, b: U }`)
/// 2. Unnamed/Tuple structs (e.g., `struct Foo(T, U)`)
/// 3. Unit structs (e.g., `struct Foo;`)
fn generate_merge_body(data_struct: &DataStruct) -> proc_macro2::TokenStream {
    match &data_struct.fields {
        Fields::Named(fields) => {
            let field_merges = fields.named.iter().map(|f| {
                let name = &f.ident;
                quote! {
                    self.#name.merge(&other.#name);
                }
            });
            quote! {
                #( #field_merges )*
            }
        }
        Fields::Unnamed(fields) => {
            let field_merges = fields.unnamed.iter().enumerate().map(|(i, _)| {
                let index = syn::Index::from(i);
                quote! {
                    self.#index.merge(&other.#index);
                }
            });
            quote! {
                #( #field_merges )*
            }
        }
        Fields::Unit => quote! {},
    }
}
