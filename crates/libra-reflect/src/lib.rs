//! libra-reflect — `#[derive(LibraReflect)]` for State and Action
//! (ARCHITECTURE.md §21): the derive that retires the hand-written
//! reflection shims and, later, drives the automation-server schema
//! (§16).
//!
//! On a **struct** (the State): implements `libra_core::ReflectState`.
//! Every named field becomes a bindable path rendered via `ToString`
//! (so any `Display` type binds). `#[libra(skip)]` excludes a field —
//! used for non-Display state like `Remote<T, E>`; per §6.3, its
//! display projection belongs on State as a plain field maintained
//! in `update`.
//!
//! On an **enum** (the Action): implements `libra_core::ReflectAction`.
//! Unit variants are constructible by handler name; payload variants
//! are effect-result events, not UI handlers, and resolve to `None`.
//! `action_names` lists every variant for diagnostics either way.
//! Variant names resolve verbatim (no case transformation) — this
//! derive predates the `#[libra::actions]` macro. The ARCHITECTURE.md
//! §3.3 camelCase public-name mapping (and the macro that generates
//! variants from snake_case method names) is MIGRATION-NAMING.md
//! Stage 4 scope, landed in both view engines together per §6.4.

mod app;
mod attrs;

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
            "LibraReflect: unions are neither State nor Action types",
        )
        .to_compile_error()
        .into(),
    }
}

/// `#[libra::state]` (ARCHITECTURE.md §5.1): a struct attribute that
/// bundles the derive set §5.1 assumes plus the `ReflectState` impl —
/// sugar over `#[derive(Debug, Clone, PartialEq, Serialize,
/// Deserialize, LibraReflect)]`.
#[proc_macro_attribute]
pub fn state(attr: TokenStream, item: TokenStream) -> TokenStream {
    attrs::state(attr, item)
}

/// `#[libra::actions]` (ARCHITECTURE.md §5.2): turns `&mut self`
/// methods into a generated `Action` enum + `reduce` function. See
/// `attrs` module docs for the exact scope of what it generates.
#[proc_macro_attribute]
pub fn actions(attr: TokenStream, item: TokenStream) -> TokenStream {
    attrs::actions(attr, item)
}

/// `libra::app!` (ARCHITECTURE.md §5.8): one declaration for the App
/// impl + view bundle + `run()` entry point. See `app` module docs for
/// scope.
#[proc_macro]
pub fn app(input: TokenStream) -> TokenStream {
    app::app(input)
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
    if !matches!(&data.fields, Fields::Named(_)) {
        return syn::Error::new_spanned(
            &input.ident,
            "LibraReflect on a State struct requires named fields",
        )
        .to_compile_error()
        .into();
    }
    derive_model_impl(&input.ident, &input.generics, &data.fields).into()
}

/// Shared by `#[derive(LibraReflect)]` (struct side) and
/// `#[libra::state]` — both need the same `ReflectState` impl, one
/// wrapping a `DeriveInput`, the other an `ItemStruct` directly.
pub(crate) fn derive_model_impl(
    name: &syn::Ident,
    generics: &syn::Generics,
    fields: &Fields,
) -> proc_macro2::TokenStream {
    let Fields::Named(named) = fields else {
        return syn::Error::new_spanned(
            name,
            "LibraReflect on a State struct requires named fields",
        )
        .to_compile_error();
    };

    let bindable: Vec<&syn::Field> =
        named.named.iter().filter(|f| !has_skip(&f.attrs)).collect();

    // Vec<T> fields can't go through `field()` — Vec has no Display —
    // so they route to `collection()` instead (§6.2: <for each={...}>
    // iteration). Everything else keeps the existing whole-value
    // Display binding.
    let (collection_fields, scalar_fields): (Vec<&syn::Field>, Vec<&syn::Field>) =
        bindable.iter().copied().partition(|f| is_vec_type(&f.ty));

    let arms = scalar_fields.iter().map(|f| {
        let ident = f.ident.as_ref().expect("named field");
        let path = ident.to_string();
        quote! {
            #path => ::core::option::Option::Some(::std::string::ToString::to_string(&self.#ident)),
        }
    });
    let names = scalar_fields.iter().map(|f| f.ident.as_ref().expect("named field").to_string());

    let collection_arms = collection_fields.iter().map(|f| {
        let ident = f.ident.as_ref().expect("named field");
        let path = ident.to_string();
        quote! {
            #path => ::core::option::Option::Some(
                self.#ident.iter().map(::std::string::ToString::to_string).collect()
            ),
        }
    });

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    quote! {
        impl #impl_generics ::libra_core::ReflectState for #name #ty_generics #where_clause {
            fn field(&self, path: &str) -> ::core::option::Option<::std::string::String> {
                match path {
                    #(#arms)*
                    _ => ::core::option::Option::None,
                }
            }
            fn field_names() -> &'static [&'static str] {
                &[#(#names),*]
            }
            fn collection(&self, path: &str) -> ::core::option::Option<::std::vec::Vec<::std::string::String>> {
                match path {
                    #(#collection_arms)*
                    _ => ::core::option::Option::None,
                }
            }
        }
    }
}

/// Syntactic `Vec<...>` detection — a macro can't resolve types, so
/// (like the derive's existing single-`String`-tuple-variant check)
/// this matches by the last path segment's identifier. A type alias
/// named `Vec` that isn't `std::vec::Vec` is a false positive that
/// fails to compile, which is the same tradeoff `is_string_type`
/// already makes in `attrs.rs`.
fn is_vec_type(ty: &syn::Type) -> bool {
    let syn::Type::Path(tp) = ty else { return false };
    tp.path.segments.last().map(|s| s.ident == "Vec").unwrap_or(false)
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
    // Type detection is by name — a macro can't resolve types; a
    // false positive fails to compile, which is the goal.
    let str_arms = single_typed_tuple_arms(name, data, "String");
    // Single-FrameInfo tuple variants are §12.5's <Frames
    // onTick={Ticked}> handlers — same shape, different payload type.
    let frame_info_arms = single_typed_tuple_arms(name, data, "FrameInfo");
    let names = data.variants.iter().map(|v| v.ident.to_string());

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    quote! {
        impl #impl_generics ::libra_core::ReflectAction for #name #ty_generics #where_clause {
            fn action_by_name(name: &str) -> ::core::option::Option<Self> {
                match name {
                    #(#ctor_arms)*
                    _ => ::core::option::Option::None,
                }
            }
            fn action_with_str(name: &str, value: ::std::string::String) -> ::core::option::Option<Self> {
                match name {
                    #(#str_arms)*
                    _ => {
                        let _ = value;
                        ::core::option::Option::None
                    }
                }
            }
            fn action_with_frame_info(name: &str, info: ::libra_core::FrameInfo) -> ::core::option::Option<Self> {
                match name {
                    #(#frame_info_arms)*
                    _ => {
                        let _ = info;
                        ::core::option::Option::None
                    }
                }
            }
            fn action_names() -> &'static [&'static str] {
                &[#(#names),*]
            }
        }
    }
    .into()
}

/// Arms for a `ReflectAction` handler method that dispatches by name
/// to single-tuple-field variants whose field type's last path segment
/// matches `ty_name` (`"String"` for `action_with_str`, `"FrameInfo"`
/// for `action_with_frame_info`). The bound variable is always named
/// `value`/`info` by the caller's match arm — this only builds the
/// `name => Some(Variant(<binding>))` arms, generic over which binding
/// identifier the surrounding method uses.
fn single_typed_tuple_arms(
    enum_name: &syn::Ident,
    data: &syn::DataEnum,
    ty_name: &str,
) -> Vec<proc_macro2::TokenStream> {
    data.variants
        .iter()
        .filter_map(|v| {
            let ident = &v.ident;
            let vname = ident.to_string();
            let Fields::Unnamed(fields) = &v.fields else { return None };
            if fields.unnamed.len() != 1 {
                return None;
            }
            let syn::Type::Path(tp) = &fields.unnamed[0].ty else { return None };
            if tp.path.segments.last().map(|s| s.ident != ty_name).unwrap_or(true) {
                return None;
            }
            let binding = if ty_name == "String" {
                quote::format_ident!("value")
            } else {
                quote::format_ident!("info")
            };
            Some(quote! {
                #vname => ::core::option::Option::Some(#enum_name::#ident(#binding)),
            })
        })
        .collect()
}
