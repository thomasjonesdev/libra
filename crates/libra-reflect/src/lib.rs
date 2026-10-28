//! libra-reflect — the proc-macro crate behind `#[derive(LibraReflect)]` and
//! the `#[libra::state]` / `#[libra::actions]` / `app!` macros: it generates
//! the `Action` enum, `reduce`, and the reflection impls an app would
//! otherwise hand-write.
//!
//! Architecture: ARCHITECTURE.md §5.2 (what the macro generates), §5.8 (app
//! assembly), §3.3 (the PascalCase → camelCase public-name mapping).

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

/// `#[libra::verifies("REQ-…")]` (ARCHITECTURE.md §15.8): a **pass-through**
/// attribute that marks a test as evidence for one or more requirement
/// ids. It expands to the annotated item unchanged — the requirement
/// string is read from *source* by `libra test --trace-matrix` (which
/// builds the requirement → test → artifact matrix), not at runtime, so
/// no registry plumbing is generated. Accepts one or more string
/// literals: `#[libra::verifies("REQ-UI-042")]` /
/// `#[libra::verifies("REQ-UI-042", "REQ-UI-043")]`.
#[proc_macro_attribute]
pub fn verifies(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Validate the shape (comma-separated string literals) so a malformed
    // annotation is a clear compile error at the call site rather than a
    // silent trace-matrix miss; then emit the item verbatim.
    let attr2: proc_macro2::TokenStream = attr.into();
    let parser = syn::punctuated::Punctuated::<syn::LitStr, syn::Token![,]>::parse_terminated;
    if let Err(e) = syn::parse::Parser::parse2(parser, attr2) {
        let mut out: proc_macro2::TokenStream = e.to_compile_error();
        out.extend(proc_macro2::TokenStream::from(item));
        return out.into();
    }
    item
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

    let bindable: Vec<&syn::Field> = named.named.iter().filter(|f| !has_skip(&f.attrs)).collect();

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
    let names = scalar_fields
        .iter()
        .map(|f| f.ident.as_ref().expect("named field").to_string());

    // PLAN-ACCESSORS: `{user.name}` — non-leaf scalar fields are assumed
    // `ReflectState + Display`; dotted tails recurse into the nested
    // impl. Leaf / generic types stay whole-value only.
    let nested_arms = scalar_fields
        .iter()
        .filter(|f| is_nestable_type(&f.ty))
        .map(|f| {
            let ident = f.ident.as_ref().expect("named field");
            let prefix = format!("{}.", ident);
            quote! {
                p if p.starts_with(#prefix) => {
                    ::libra::ReflectState::field(&self.#ident, &p[#prefix.len()..])
                }
            }
        });

    // PLAN-ACCESSORS: `{items.len}` / `{name.isEmpty}` (optional trailing
    // `()` folded away below). Whitelist only — not arbitrary methods.
    let string_accessor_arms = scalar_fields
        .iter()
        .filter(|f| path_ends_with(&f.ty, "String"))
        .flat_map(|f| {
            let ident = f.ident.as_ref().expect("named field");
            let len = format!("{ident}.len");
            let empty = format!("{ident}.isEmpty");
            [
                quote! {
                    #len => ::core::option::Option::Some(self.#ident.len().to_string()),
                },
                quote! {
                    #empty => ::core::option::Option::Some(self.#ident.is_empty().to_string()),
                },
            ]
        });
    let vec_accessor_arms = collection_fields.iter().flat_map(|f| {
        let ident = f.ident.as_ref().expect("named field");
        let len = format!("{ident}.len");
        let empty = format!("{ident}.isEmpty");
        [
            quote! {
                #len => ::core::option::Option::Some(self.#ident.len().to_string()),
            },
            quote! {
                #empty => ::core::option::Option::Some(self.#ident.is_empty().to_string()),
            },
        ]
    });

    let collection_arms = collection_fields.iter().map(|f| {
        let ident = f.ident.as_ref().expect("named field");
        let path = ident.to_string();
        quote! {
            #path => ::core::option::Option::Some(
                self.#ident.iter().map(::std::string::ToString::to_string).collect()
            ),
        }
    });
    let collection_row_arms = collection_fields.iter().map(|f| {
        let ident = f.ident.as_ref().expect("named field");
        let path = ident.to_string();
        // Opaque element types stay Display-only (dashboard SeriesHandle,
        // Vec<String>). Everything else is assumed ReflectState+Display
        // so `{t.field}` works (PLAN-ITEM-FIELDS).
        let row_fn = if is_opaque_vec_element(&f.ty) {
            quote! { ::libra::collection_row_opaque }
        } else {
            quote! { ::libra::collection_row_from_state }
        };
        quote! {
            #path => ::core::option::Option::Some(
                self.#ident.iter().map(#row_fn).collect()
            ),
        }
    });

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    quote! {
        impl #impl_generics ::libra::ReflectState for #name #ty_generics #where_clause {
            fn field(&self, path: &str) -> ::core::option::Option<::std::string::String> {
                // `{items.len()}` and `{items.len}` are the same binding.
                let path = path.strip_suffix("()").unwrap_or(path);
                match path {
                    #(#arms)*
                    #(#string_accessor_arms)*
                    #(#vec_accessor_arms)*
                    #(#nested_arms)*
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
            fn collection_rows(
                &self,
                path: &str,
            ) -> ::core::option::Option<::std::vec::Vec<::libra::CollectionRow>> {
                match path {
                    #(#collection_row_arms)*
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
    let syn::Type::Path(tp) = ty else {
        return false;
    };
    tp.path
        .segments
        .last()
        .map(|s| s.ident == "Vec")
        .unwrap_or(false)
}

/// Element types that must stay whole-value Display rows (no field map).
/// `String` and `SeriesHandle` are the two that ship today; anything
/// else is treated as a structured `ReflectState` item.
fn is_opaque_vec_element(ty: &syn::Type) -> bool {
    let Some(inner) = vec_element_type(ty) else {
        return true;
    };
    path_ends_with(inner, "String") || path_ends_with(inner, "SeriesHandle")
}

fn vec_element_type(ty: &syn::Type) -> Option<&syn::Type> {
    let syn::Type::Path(tp) = ty else { return None };
    let seg = tp.path.segments.last()?;
    if seg.ident != "Vec" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
        return None;
    };
    args.args.iter().find_map(|a| match a {
        syn::GenericArgument::Type(t) => Some(t),
        _ => None,
    })
}

fn path_ends_with(ty: &syn::Type, name: &str) -> bool {
    let syn::Type::Path(tp) = ty else {
        return false;
    };
    tp.path.segments.last().is_some_and(|s| s.ident == name)
}

/// Types that may carry nested `ReflectState` for `{user.name}`
/// (PLAN-ACCESSORS). Leaf scalars and any generic (`Remote<T,E>`,
/// `Option<T>`, …) stay whole-value Display only — the macro can't
/// prove ReflectState, and generating a nested arm would fail to compile.
fn is_nestable_type(ty: &syn::Type) -> bool {
    let syn::Type::Path(tp) = ty else {
        return false;
    };
    let Some(seg) = tp.path.segments.last() else {
        return false;
    };
    if !matches!(seg.arguments, syn::PathArguments::None) {
        return false;
    }
    !is_leaf_ident(&seg.ident)
}

fn is_leaf_ident(ident: &syn::Ident) -> bool {
    matches!(
        ident.to_string().as_str(),
        "String"
            | "bool"
            | "SeriesHandle"
            | "FrameInfo"
            | "Instant"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "f32"
            | "f64"
    )
}

fn derive_msg(input: &DeriveInput, data: &syn::DataEnum) -> TokenStream {
    let name = &input.ident;

    let ctor_arms = data.variants.iter().filter_map(|v| {
        let ident = &v.ident;
        let public_name = pascal_to_camel(&ident.to_string());
        match v.fields {
            Fields::Unit => Some(quote! {
                #public_name => ::core::option::Option::Some(#name::#ident),
            }),
            // Payload variants are effect results — not nameable handlers.
            _ => None,
        }
    });
    // Single-String tuple variants are control-payload handlers
    // (`onChange={titleChanged}`): the control provides the value.
    // Type detection is by name — a macro can't resolve types; a
    // false positive fails to compile, which is the goal.
    let str_arms = single_typed_tuple_arms(name, data, "String", "value");
    // Single-FrameInfo tuple variants are §12.5's <Frames
    // onTick={ticked}> handlers — same shape, different payload type.
    let frame_info_arms = single_typed_tuple_arms(name, data, "FrameInfo", "info");
    // Single-Instant tuple variants are §12.5's
    // <Interval onTick={ticked}> handlers — same shape again.
    let instant_arms = single_typed_tuple_arms(name, data, "Instant", "instant");
    let stream_arms = single_typed_tuple_arms(name, data, "StreamEvent", "event");
    let names = data
        .variants
        .iter()
        .map(|v| pascal_to_camel(&v.ident.to_string()));

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    quote! {
        impl #impl_generics ::libra::ReflectAction for #name #ty_generics #where_clause {
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
            fn action_with_frame_info(name: &str, info: ::libra::FrameInfo) -> ::core::option::Option<Self> {
                match name {
                    #(#frame_info_arms)*
                    _ => {
                        let _ = info;
                        ::core::option::Option::None
                    }
                }
            }
            fn action_with_instant(name: &str, instant: ::libra::Instant) -> ::core::option::Option<Self> {
                match name {
                    #(#instant_arms)*
                    _ => {
                        let _ = instant;
                        ::core::option::Option::None
                    }
                }
            }
            fn action_with_stream_event(name: &str, event: ::libra::StreamEvent) -> ::core::option::Option<Self> {
                match name {
                    #(#stream_arms)*
                    _ => {
                        let _ = event;
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
/// for `action_with_frame_info`, `"Instant"` for `action_with_instant`).
/// `binding` is the bound variable name the caller's match arm already
/// uses (`value`/`info`/`instant`) — this only builds the
/// `name => Some(Variant(<binding>))` arms, generic over which binding
/// identifier the surrounding method uses.
fn single_typed_tuple_arms(
    enum_name: &syn::Ident,
    data: &syn::DataEnum,
    ty_name: &str,
    binding: &str,
) -> Vec<proc_macro2::TokenStream> {
    let binding = quote::format_ident!("{binding}");
    data.variants
        .iter()
        .filter_map(|v| {
            let ident = &v.ident;
            let public_name = pascal_to_camel(&ident.to_string());
            let Fields::Unnamed(fields) = &v.fields else {
                return None;
            };
            if fields.unnamed.len() != 1 {
                return None;
            }
            let syn::Type::Path(tp) = &fields.unnamed[0].ty else {
                return None;
            };
            if tp
                .path
                .segments
                .last()
                .map(|s| s.ident != ty_name)
                .unwrap_or(true)
            {
                return None;
            }
            Some(quote! {
                #public_name => ::core::option::Option::Some(#enum_name::#ident(#binding)),
            })
        })
        .collect()
}

/// Rust variant identifier → camelCase public name (§3.3): lowercase
/// just the first character — `#[libra::actions]`'s generated variant
/// idents are already correctly cased past the first letter (built
/// from a snake_case method name), and this derive holds hand-written
/// enums to the identical public surface, so both paths resolve the
/// same markup handler names.
fn pascal_to_camel(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
