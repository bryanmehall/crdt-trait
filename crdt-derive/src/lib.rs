extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
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

/// The entry point for the `DeltaSync` derive procedural macro.
///
/// Generates a `DeltaSync` implementation for structs where all fields implement
/// `DeltaSync`. A companion delta struct `{Name}Delta` is generated, along with
/// `Crdt` and `DeltaSync` implementations.
///
/// The summary type is a tuple of per-field summaries.
#[proc_macro_derive(DeltaSync)]
pub fn derive_delta_sync(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match generate_delta_sync_impl(input) {
        Ok(token_stream) => token_stream.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Orchestrates the generation of the `Crdt` trait implementation.
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

/// Orchestrates the generation of the `DeltaSync` trait implementation.
fn generate_delta_sync_impl(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let generics = &input.generics;
    let delta_name = format_ident!("{}Delta", name);

    let data_struct = match &input.data {
        Data::Struct(data_struct) => data_struct,
        Data::Enum(_) => {
            return Err(syn::Error::new(
                name.span(),
                "Derive(DeltaSync) is currently only supported for structs.",
            ));
        }
        Data::Union(_) => {
            return Err(syn::Error::new(
                name.span(),
                "Derive(DeltaSync) is not supported for unions.",
            ));
        }
    };

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    match &data_struct.fields {
        Fields::Named(fields) => {
            let field_names: Vec<_> = fields
                .named
                .iter()
                .map(|f| f.ident.as_ref().unwrap())
                .collect();
            let field_types: Vec<_> = fields.named.iter().map(|f| &f.ty).collect();

            // Summary type: tuple of per-field summaries
            let summary_type = quote! {
                ( #( <#field_types as crdt::DeltaSync>::Summary ),* )
            };

            // Generate indices for tuple access in summary
            let indices: Vec<syn::Index> =
                (0..field_names.len()).map(syn::Index::from).collect();

            Ok(quote! {
                // Generated delta struct
                #[derive(Debug, Clone, PartialEq, Default)]
                pub struct #delta_name #generics {
                    #( pub #field_names: <#field_types as crdt::DeltaSync>::Delta ),*
                }

                // Crdt impl for the delta struct (product merge)
                impl #impl_generics crdt::Crdt for #delta_name #ty_generics #where_clause {
                    type Value = Self;

                    fn merge(&mut self, other: &Self) {
                        #( crdt::Crdt::merge(&mut self.#field_names, &other.#field_names); )*
                    }

                    fn value(&self) -> Self::Value {
                        self.clone()
                    }
                }

                // DeltaSync impl for the original struct
                impl #impl_generics crdt::DeltaSync for #name #ty_generics #where_clause {
                    type Summary = #summary_type;
                    type Delta = #delta_name #ty_generics;

                    fn summary(&self) -> Self::Summary {
                        ( #( crdt::DeltaSync::summary(&self.#field_names) ),* )
                    }

                    fn delta_from_summary(&self, remote_summary: &Self::Summary) -> Self::Delta {
                        #delta_name {
                            #( #field_names: crdt::DeltaSync::delta_from_summary(&self.#field_names, &remote_summary.#indices) ),*
                        }
                    }

                    fn merge_delta(&mut self, delta: &Self::Delta) {
                        #( crdt::DeltaSync::merge_delta(&mut self.#field_names, &delta.#field_names); )*
                    }
                }
            })
        }
        Fields::Unnamed(fields) => {
            let field_types: Vec<_> = fields.unnamed.iter().map(|f| &f.ty).collect();
            let indices: Vec<syn::Index> =
                (0..fields.unnamed.len()).map(syn::Index::from).collect();

            let summary_type = quote! {
                ( #( <#field_types as crdt::DeltaSync>::Summary ),* )
            };

            Ok(quote! {
                // Generated delta struct (tuple variant)
                #[derive(Debug, Clone, PartialEq, Default)]
                pub struct #delta_name #generics (
                    #( pub <#field_types as crdt::DeltaSync>::Delta ),*
                );

                impl #impl_generics crdt::Crdt for #delta_name #ty_generics #where_clause {
                    type Value = Self;

                    fn merge(&mut self, other: &Self) {
                        #( crdt::Crdt::merge(&mut self.#indices, &other.#indices); )*
                    }

                    fn value(&self) -> Self::Value {
                        self.clone()
                    }
                }

                impl #impl_generics crdt::DeltaSync for #name #ty_generics #where_clause {
                    type Summary = #summary_type;
                    type Delta = #delta_name #ty_generics;

                    fn summary(&self) -> Self::Summary {
                        ( #( crdt::DeltaSync::summary(&self.#indices) ),* )
                    }

                    fn delta_from_summary(&self, remote_summary: &Self::Summary) -> Self::Delta {
                        #delta_name (
                            #( crdt::DeltaSync::delta_from_summary(&self.#indices, &remote_summary.#indices) ),*
                        )
                    }

                    fn merge_delta(&mut self, delta: &Self::Delta) {
                        #( crdt::DeltaSync::merge_delta(&mut self.#indices, &delta.#indices); )*
                    }
                }
            })
        }
        Fields::Unit => Ok(quote! {
            impl #impl_generics crdt::DeltaSync for #name #ty_generics #where_clause {
                type Summary = ();
                type Delta = Self;

                fn summary(&self) -> Self::Summary {
                    ()
                }

                fn delta_from_summary(&self, _remote_summary: &Self::Summary) -> Self::Delta {
                    Self::default()
                }

                fn merge_delta(&mut self, _delta: &Self::Delta) {}
            }
        }),
    }
}

/// Generates the code for the `merge` method's body based on the struct fields.
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
