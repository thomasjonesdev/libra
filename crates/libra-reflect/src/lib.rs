//! libra-reflect — `#[derive(LibraReflect)]` for Model and Msg
//! (ARCHITECTURE.md §21): the derive that retires the hand-written
//! reflection shims and, later, drives the automation-server schema
//! (§16).
//!
//! On a **struct** (the Model): implements `libra_core::ReflectModel`.
//! Every named field becomes a bindable path rendered via `ToString`
//! (so any `Display` type binds). `#[libra(skip)]` excludes a field —
//! used for non-Display state like `Remote<T, E>`; per §6.3, its
//! display projection belongs on the Model as a plain field maintained
//! in `update`.
//!
//! On an **enum** (the Msg): implements `libra_core::ReflectMsg`. Unit
//! variants are constructible by handler name; payload variants are
//! effect-result events, not UI handlers, and resolve to `None`.
//! `msg_names` lists every variant for diagnostics either way.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

#[proc_macro_derive(LibraReflect, attributes(libra))]
pub fn derive_libra_reflect(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match &input.data {
        Data::Struct(s) => derive_model(&input, s),
        Data::Enum(e) => derive_msg(&input, e),
        Data::Union(_) => syn::Error::new_spanned(
            &input.ident,
            "LibraReflect: unions are neither Models nor Msgs",
        )
        .to_compile_error()
        .into(),
    }
}

fn has_skip(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        if !a.path().is_ident("libra") {
            return false;
        }
        let mut skip = false;
        // #[libra(skip)] — anything else under #[libra(...)] is reserved.
        let _ = a.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                skip = true;
            }
            Ok(())
        });
        skip
    })
}

fn derive_model(input: &DeriveInput, data: &syn::DataStruct) -> TokenStream {
    let name = &input.ident;
    let Fields::Named(fields) = &data.fields else {
        return syn::Error::new_spanned(
            name,
            "LibraReflect on a Model requires named fields",
        )
        .to_compile_error()
        .into();
    };

    let bindable: Vec<&syn::Field> =
        fields.named.iter().filter(|f| !has_skip(&f.attrs)).collect();

    let arms = bindable.iter().map(|f| {
        let ident = f.ident.as_ref().expect("named field");
        let path = ident.to_string();
        quote! {
            #path => ::core::option::Option::Some(::std::string::ToString::to_string(&self.#ident)),
        }
    });
    let names = bindable.iter().map(|f| f.ident.as_ref().expect("named field").to_string());

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    quote! {
        impl #impl_generics ::libra_core::ReflectModel for #name #ty_generics #where_clause {
            fn field(&self, path: &str) -> ::core::option::Option<::std::string::String> {
                match path {
                    #(#arms)*
                    _ => ::core::option::Option::None,
                }
            }
            fn field_names() -> &'static [&'static str] {
                &[#(#names),*]
            }
        }
    }
    .into()
}

fn derive_msg(input: &DeriveInput, data: &syn::DataEnum) -> TokenStream {
    let name = &input.ident;

    let ctor_arms = data.variants.iter().filter_map(|v| {
        let ident = &v.ident;
        let vname = ident.to_string();
        match v.fields {
            Fields::Unit => Some(quote! {
                #vname => ::core::option::Option::Some(#name::#ident),
            }),
            // Payload variants are effect results — not nameable handlers.
            _ => None,
        }
    });
    // Single-String tuple variants are control-payload handlers
    // (`onChange={TitleChanged}`): the control provides the value.
    // Type detection is by name (`String`) — a macro can't resolve
    // types; a false positive fails to compile, which is the goal.
    let str_arms = data.variants.iter().filter_map(|v| {
        let ident = &v.ident;
        let vname = ident.to_string();
        let Fields::Unnamed(fields) = &v.fields else { return None };
        if fields.unnamed.len() != 1 {
            return None;
        }
        let syn::Type::Path(tp) = &fields.unnamed[0].ty else { return None };
        if tp.path.segments.last().map(|s| s.ident != "String").unwrap_or(true) {
            return None;
        }
        Some(quote! {
            #vname => ::core::option::Option::Some(#name::#ident(value)),
        })
    });
    let names = data.variants.iter().map(|v| v.ident.to_string());

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    quote! {
        impl #impl_generics ::libra_core::ReflectMsg for #name #ty_generics #where_clause {
            fn msg_by_name(name: &str) -> ::core::option::Option<Self> {
                match name {
                    #(#ctor_arms)*
                    _ => ::core::option::Option::None,
                }
            }
            fn msg_with_str(name: &str, value: ::std::string::String) -> ::core::option::Option<Self> {
                match name {
                    #(#str_arms)*
                    _ => {
                        let _ = value;
                        ::core::option::Option::None
                    }
                }
            }
            fn msg_names() -> &'static [&'static str] {
                &[#(#names),*]
            }
        }
    }
    .into()
}
