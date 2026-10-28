//! `#[libra::state]` / `#[libra::actions]` — ARCHITECTURE.md §5.1–§5.2.
//!
//! Sugar over the same substrate the framework has always run on: a
//! `State` struct, a serializable `Action` enum, and a pure
//! `reduce(state, action) -> Vec<Effect<Action>>`. `#[libra::actions]`
//! turns plain `&mut self` methods into that enum + reducer instead of
//! requiring them hand-written; §5.2's expansion example is exactly
//! what these two macros emit.
//!
//! Scope, honestly stated (MIGRATION-NAMING.md Stage 4):
//! - One `#[libra::actions]` block per module — the enum is named
//!   `Action` and the reducer `reduce`, both fixed names. Slices
//!   (§5.7, composing several actions blocks under one root) are not
//!   implemented yet.
//! - The dispatchability rule (§5.2) is scoped to what `ReflectAction`
//!   already exposes: zero-arg methods are unit-variant handlers
//!   (`action_by_name`), single-`String`-arg methods are handlers with
//!   a control payload (`action_with_str`), single-`FrameInfo`-arg
//!   methods are handlers for §12.5's `<Frames onTick={name}>` source
//!   element (`action_with_frame_info`), single-`Instant`-arg methods
//!   are handlers for `<Interval every={ms} onTick={name}>`
//!   (`action_with_instant`), single-`StreamEvent`-arg methods are
//!   handlers for `<Stream onEvent={name}>` (`action_with_stream_event`).
//!   Any other signature is a
//!   continuation — constructible only by the runtime delivering that
//!   effect result, never nameable from markup. Growing markup's
//!   payload whitelist further (bool, ids, …) is tracked as an open
//!   question in ARCHITECTURE.md §26 and is a separate change to
//!   `ReflectAction` plus both view engines, not this macro.
//! - Every method must take `&mut self` as its receiver; anything else
//!   is a compile error naming the offending method.
//! - One method may be marked `#[project]` (§6.3's display-projection
//!   hook): exactly `fn name(&mut self)`. It is not an action — never
//!   enters the enum — and the generated `reduce` calls it after every
//!   arm, so derived display fields are rebuilt by the framework
//!   instead of by per-action discipline. `__libra_project()` (the
//!   always-emitted internal entry point, no-op without a hook) lets
//!   `libra::app!`'s no-`init:` default project `Default::default()`
//!   so derived fields are correct from frame zero.
//! - `#[libra::state]` bundles `#[derive(Serialize, Deserialize)]`,
//!   which requires the crate to depend on `serde` (`features =
//!   ["derive"]`) directly — serde's derive expands to `extern crate
//!   serde as _serde`, so this can't be hidden behind a re-export.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, ImplItem, ItemImpl, ItemStruct, PatType, Token, Type};

use crate::derive_model_impl;

pub fn state(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut item_struct = parse_macro_input!(item as ItemStruct);
    let reflect_impl = derive_model_impl(
        &item_struct.ident,
        &item_struct.generics,
        &item_struct.fields,
    );

    // Inject the derive bundle §5.1 assumes: Clone (time travel, §5.6),
    // Debug + PartialEq (test assertions), Serialize/Deserialize
    // (snapshots, rehydration, session export). Requires the crate to
    // depend on `serde` directly (features = ["derive"]) — same
    // requirement as writing `#[derive(Serialize)]` by hand; serde's
    // derive expands to `extern crate serde as _serde`, so no amount
    // of re-exporting through libra-core avoids it. A struct that
    // can't satisfy these (a field with no serde impl, say) should
    // skip this macro and hand-roll — §5.1 says the expansion is
    // always available as a fallback.
    let derive_attr: syn::Attribute = syn::parse_quote! {
        #[derive(::std::fmt::Debug, ::std::clone::Clone, ::std::cmp::PartialEq, ::serde::Serialize, ::serde::Deserialize)]
    };
    item_struct.attrs.insert(0, derive_attr);

    // Attribute macros replace the whole item, unlike derive macros —
    // rustc never sees the original tokens, so any inert helper
    // attribute we read here (`#[libra(skip)]`, consumed above by
    // derive_model_impl's has_skip check) must be stripped before
    // re-emitting, or rustc rejects it as an attribute no macro
    // registered.
    if let syn::Fields::Named(named) = &mut item_struct.fields {
        for field in &mut named.named {
            field.attrs.retain(|a| !a.path().is_ident("libra"));
        }
    }

    let rehydrate_impl = rehydrate_impl(&item_struct);

    quote! {
        #item_struct
        #reflect_impl
        #rehydrate_impl
    }
    .into()
}

/// §18.2: restore a snapshot field by field. `#[serde(skip)]` and
/// `#[serde(skip_deserializing)]` stay at `Default` and are not reported.
/// `#[libra(skip)]` is reflection-only — those fields are still serde
/// data unless a serde skip says otherwise.
fn rehydrate_impl(item: &syn::ItemStruct) -> proc_macro2::TokenStream {
    let syn::Fields::Named(named) = &item.fields else {
        return quote! {};
    };
    let name = &item.ident;
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();
    let mut keys = Vec::new();
    let mut ok_fields = Vec::new();
    let mut default_fields = Vec::new();
    for field in &named.named {
        let Some(ident) = &field.ident else {
            continue;
        };
        let (rename, skip) = serde_restore_meta(field);
        default_fields.push(quote! { #ident: ::core::default::Default::default() });
        if skip {
            ok_fields.push(quote! { #ident: ::core::default::Default::default() });
        } else {
            let key = rename.unwrap_or_else(|| ident.to_string());
            keys.push(key.clone());
            ok_fields.push(quote! {
                #ident: ::libra::take_field(&obj, #key, &mut report)
            });
        }
    }
    quote! {
        impl #impl_generics ::libra::Rehydrate for #name #ty_generics #where_clause {
            fn rehydrate(json: &str) -> (Self, ::libra::RehydrateReport) {
                let mut report = ::libra::RehydrateReport::default();
                let Ok(obj) = ::libra::snapshot_object(json) else {
                    report.invalid = true;
                    return (Self { #(#default_fields),* }, report);
                };
                ::libra::note_removed(&obj, &[#(#keys),*] as &[&str], &mut report);
                (Self { #(#ok_fields),* }, report)
            }
        }
    }
}

/// `(rename, skip_deserialize)` from `#[serde(...)]` on a state field.
fn serde_restore_meta(field: &syn::Field) -> (Option<String>, bool) {
    let mut rename = None;
    let mut skip = false;
    for attr in &field.attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                let value: syn::LitStr = meta.value()?.parse()?;
                rename = Some(value.value());
            } else if meta.path.is_ident("skip") || meta.path.is_ident("skip_deserializing") {
                skip = true;
            } else if meta.input.peek(Token![=]) {
                let _value: syn::Expr = meta.value()?.parse()?;
            }
            Ok(())
        });
    }
    (rename, skip)
}

pub fn actions(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut item_impl = parse_macro_input!(item as ItemImpl);
    let state_ty = item_impl.self_ty.clone();
    let state_ty = &state_ty;

    let mut variants = Vec::new(); // enum variant defs
    let mut reduce_arms = Vec::new();
    let mut by_name_arms = Vec::new();
    let mut with_str_arms = Vec::new();
    let mut with_frame_info_arms = Vec::new();
    let mut with_instant_arms = Vec::new();
    let mut with_stream_arms = Vec::new();
    let mut all_names = Vec::new();
    let mut errors = Vec::new();
    // §6.3's display-projection hook: the one `#[project]`-marked method,
    // called by the generated `reduce` after every action so derived
    // fields can never go stale. Not an action — never enters the enum.
    let mut project_fn: Option<syn::Ident> = None;

    for item in &mut item_impl.items {
        let ImplItem::Fn(method) = item else { continue };
        if let Some(pos) = method
            .attrs
            .iter()
            .position(|a| a.path().is_ident("project"))
        {
            // Inert helper attr — strip before re-emitting (rustc knows
            // no `#[project]`), same treatment as `#[libra(skip)]`.
            method.attrs.remove(pos);
            let ok_receiver = matches!(
                method.sig.inputs.first(),
                Some(FnArg::Receiver(r)) if r.mutability.is_some() && r.reference.is_some()
            );
            let no_params = method.sig.inputs.len() == 1;
            let no_return = matches!(method.sig.output, syn::ReturnType::Default);
            if !(ok_receiver && no_params && no_return) {
                errors.push(syn::Error::new_spanned(
                    &method.sig,
                    "#[project]: the projection hook must be exactly `fn name(&mut self)` — \
                     no other parameters, no return value (it rebuilds derived display fields, §6.3)",
                ));
                continue;
            }
            if let Some(existing) = &project_fn {
                errors.push(syn::Error::new_spanned(
                    &method.sig,
                    format!(
                        "#[project]: only one projection hook per actions block — '{existing}' is already marked"
                    ),
                ));
                continue;
            }
            project_fn = Some(method.sig.ident.clone());
            continue; // not an action: no variant, no reduce arm, no name
        }
        let method_ident = &method.sig.ident;
        let variant_ident = syn::Ident::new(
            &snake_to_pascal(&method_ident.to_string()),
            method_ident.span(),
        );
        let public_name = pascal_to_camel(&snake_to_pascal(&method_ident.to_string()));

        let mut inputs = method.sig.inputs.iter();
        match inputs.next() {
            Some(FnArg::Receiver(r)) if r.mutability.is_some() && r.reference.is_some() => {}
            _ => {
                errors.push(syn::Error::new_spanned(
                    &method.sig,
                    format!(
                        "#[libra::actions]: '{method_ident}' must take &mut self — every action is a state transition"
                    ),
                ));
                continue;
            }
        }

        let params: Vec<&PatType> = inputs
            .filter_map(|a| match a {
                FnArg::Typed(pt) => Some(pt),
                FnArg::Receiver(_) => None,
            })
            .collect();

        all_names.push(public_name.clone());

        match params.as_slice() {
            [] => {
                variants.push(quote! { #variant_ident });
                reduce_arms.push(quote! {
                    Action::#variant_ident => ::libra::IntoEffects::into_effects(state.#method_ident()),
                });
                by_name_arms.push(quote! {
                    #public_name => ::core::option::Option::Some(Action::#variant_ident),
                });
            }
            [p] if is_string_type(&p.ty) => {
                variants.push(quote! { #variant_ident(::std::string::String) });
                reduce_arms.push(quote! {
                    Action::#variant_ident(value) => ::libra::IntoEffects::into_effects(state.#method_ident(value)),
                });
                with_str_arms.push(quote! {
                    #public_name => ::core::option::Option::Some(Action::#variant_ident(value)),
                });
            }
            [p] if is_frame_info_type(&p.ty) => {
                // §12.5's <Frames onTick={name}> source element: not a
                // continuation (unlike other non-String single-arg
                // methods) — the runtime constructs this by *name* on
                // every tick, the same handler-dispatch shape as
                // action_with_str, just for a different payload type.
                variants.push(quote! { #variant_ident(::libra::FrameInfo) });
                reduce_arms.push(quote! {
                    Action::#variant_ident(info) => ::libra::IntoEffects::into_effects(state.#method_ident(info)),
                });
                with_frame_info_arms.push(quote! {
                    #public_name => ::core::option::Option::Some(Action::#variant_ident(info)),
                });
            }
            [p] if is_stream_event_type(&p.ty) => {
                variants.push(quote! { #variant_ident(::libra::StreamEvent) });
                reduce_arms.push(quote! {
                    Action::#variant_ident(event) => ::libra::IntoEffects::into_effects(state.#method_ident(event)),
                });
                with_stream_arms.push(quote! {
                    #public_name => ::core::option::Option::Some(Action::#variant_ident(event)),
                });
            }
            [p] if is_instant_type(&p.ty) => {
                // §12.5's <Interval every={ms} onTick={name}> source
                // element — same shape as the FrameInfo arm above, just
                // a different runtime-supplied payload type.
                variants.push(quote! { #variant_ident(::libra::Instant) });
                reduce_arms.push(quote! {
                    Action::#variant_ident(instant) => ::libra::IntoEffects::into_effects(state.#method_ident(instant)),
                });
                with_instant_arms.push(quote! {
                    #public_name => ::core::option::Option::Some(Action::#variant_ident(instant)),
                });
            }
            params => {
                // Continuation: not nameable from markup (§5.2's
                // dispatchability rule) — constructed only via the
                // effect result the runtime delivers.
                let tys: Vec<&Type> = params.iter().map(|p| p.ty.as_ref()).collect();
                let binds: Vec<syn::Ident> = (0..params.len())
                    .map(|i| syn::Ident::new(&format!("p{i}"), method_ident.span()))
                    .collect();
                variants.push(quote! { #variant_ident(#(#tys),*) });
                reduce_arms.push(quote! {
                    Action::#variant_ident(#(#binds),*) => ::libra::IntoEffects::into_effects(state.#method_ident(#(#binds),*)),
                });
            }
        }
    }

    if !errors.is_empty() {
        return errors
            .into_iter()
            .map(|e| e.to_compile_error())
            .collect::<proc_macro2::TokenStream>()
            .into();
    }

    let unmatched_str = if with_str_arms.is_empty() {
        quote! { let _ = value; }
    } else {
        quote! {}
    };
    let unmatched_frame_info = if with_frame_info_arms.is_empty() {
        quote! { let _ = info; }
    } else {
        quote! {}
    };
    let unmatched_instant = if with_instant_arms.is_empty() {
        quote! { let _ = instant; }
    } else {
        quote! {}
    };
    let unmatched_stream = if with_stream_arms.is_empty() {
        quote! { let _ = event; }
    } else {
        quote! {}
    };

    // §6.3: when a `#[project]` hook exists, every reduce ends with it —
    // handlers and effect-result continuations alike — so display
    // projections are rebuilt by the framework, not by per-action
    // discipline. `__libra_project` is the always-present internal entry
    // point (no-op without a hook): `libra::app!`'s no-`init:` default
    // runs it on `Default::default()` so derived fields are correct from
    // frame zero.
    let project_call = project_fn.as_ref().map(|f| quote! { state.#f(); });
    let project_body = project_fn.as_ref().map(|f| quote! { self.#f(); });

    quote! {
        #item_impl

        #[derive(::std::fmt::Debug, ::std::clone::Clone, ::std::cmp::PartialEq)]
        pub enum Action {
            #(#variants),*
        }

        impl #state_ty {
            /// Generated by `#[libra::actions]` (§5.2/§6.3): runs the
            /// `#[project]` hook if one is declared (no-op otherwise).
            /// `libra::app!` calls this on the default state when no
            /// `init:` field is given. Hidden: apps never call it —
            /// `reduce` already projects after every action.
            #[doc(hidden)]
            pub fn __libra_project(&mut self) {
                #project_body
            }
        }

        /// The reducer (§5.2): the only place state changes. Generated
        /// verbatim from the `#[libra::actions]` impl above — every arm
        /// is one method call plus `IntoEffects::into_effects`, followed
        /// by the `#[project]` hook when one is declared (§6.3).
        pub fn reduce(state: &mut #state_ty, action: Action) -> ::std::vec::Vec<::libra::Effect<Action>> {
            let effects = match action {
                #(#reduce_arms)*
            };
            #project_call
            effects
        }

        impl ::libra::ReflectAction for Action {
            fn action_by_name(name: &str) -> ::core::option::Option<Self> {
                match name {
                    #(#by_name_arms)*
                    _ => ::core::option::Option::None,
                }
            }
            fn action_with_str(name: &str, value: ::std::string::String) -> ::core::option::Option<Self> {
                #unmatched_str
                match name {
                    #(#with_str_arms)*
                    _ => ::core::option::Option::None,
                }
            }
            fn action_with_frame_info(name: &str, info: ::libra::FrameInfo) -> ::core::option::Option<Self> {
                #unmatched_frame_info
                match name {
                    #(#with_frame_info_arms)*
                    _ => ::core::option::Option::None,
                }
            }
            fn action_with_instant(name: &str, instant: ::libra::Instant) -> ::core::option::Option<Self> {
                #unmatched_instant
                match name {
                    #(#with_instant_arms)*
                    _ => ::core::option::Option::None,
                }
            }
            fn action_with_stream_event(name: &str, event: ::libra::StreamEvent) -> ::core::option::Option<Self> {
                #unmatched_stream
                match name {
                    #(#with_stream_arms)*
                    _ => ::core::option::Option::None,
                }
            }
            fn action_names() -> &'static [&'static str] {
                &[#(#all_names),*]
            }
        }
    }
    .into()
}

fn is_string_type(ty: &Type) -> bool {
    let Type::Path(tp) = ty else { return false };
    tp.path
        .segments
        .last()
        .map(|s| s.ident == "String")
        .unwrap_or(false)
}

fn is_frame_info_type(ty: &Type) -> bool {
    let Type::Path(tp) = ty else { return false };
    tp.path
        .segments
        .last()
        .map(|s| s.ident == "FrameInfo")
        .unwrap_or(false)
}

fn is_instant_type(ty: &Type) -> bool {
    let Type::Path(tp) = ty else { return false };
    tp.path
        .segments
        .last()
        .map(|s| s.ident == "Instant")
        .unwrap_or(false)
}

fn is_stream_event_type(ty: &Type) -> bool {
    let Type::Path(tp) = ty else { return false };
    tp.path
        .segments
        .last()
        .map(|s| s.ident == "StreamEvent")
        .unwrap_or(false)
}

fn snake_to_pascal(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut c = part.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn pascal_to_camel(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
