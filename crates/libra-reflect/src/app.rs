//! `libra::app!` (ARCHITECTURE.md §5.8): one declaration replacing the
//! wiring boilerplate — the `App` impl, the view bundle, and a `run()`
//! entry point — around a `#[libra::state]` struct and a
//! `#[libra::actions]` impl block in the same module.
//!
//! Scope, honestly stated (PLAN-ERGONOMICS.md E1):
//! - Assumes exactly one `#[libra::actions]` block in the invoking
//!   module (references the fixed names `Action`/`reduce` it
//!   generates) — the same one-block-per-module constraint
//!   `#[libra::actions]` itself documents.
//! - "The view bundle" (`VIEW`) is `include_str!` — the same static
//!   embedding `examples/*/src/lib.rs` already hand-writes today, and
//!   what release/installed binaries render from. In addition, the
//!   generated `run()` watches that `.lx` file for edits in *debug*
//!   builds when it's present on disk (running from the source tree),
//!   so `cargo run -p myapp` hot-reloads with state preserved out of
//!   the box. The watch path is absolute (via `CARGO_MANIFEST_DIR`),
//!   so it is CWD-independent; release builds never reference it.
//!   Full dev-server tooling (`libra dev`, §18.1) is still separate.
//! - Emits a `run()` function, not a literal `fn main()` — a
//!   function-like proc macro has no way to detect "is this crate's
//!   target a binary", so `main.rs` stays a one-line call to it
//!   (`fn main() { myapp::run().unwrap() }`), same shape as today's
//!   examples but with the `impl App` and `pub const VIEW` gone.
//! - No `init:` field ⇒ falls back to `Default::default()` with no
//!   startup effects; if the state type isn't `Default`, that's a
//!   plain rustc error naming the missing bound — no macro-level
//!   magic, matching §6.5's "typos become compile errors" posture.
//! - Optional `subscriptions: SomeFn` field, for apps with ambient
//!   event sources beyond what markup can declare today — §12.5's
//!   source elements can't yet be conditionally mounted (no `<if>` in
//!   the grammar, §26), so anything state-dependent (a dashboard's
//!   "only tick while running") still needs a hand-written
//!   `fn(&State) -> Vec<Sub<Action>>`.

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Expr, Ident, LitStr, Path, Token};

enum Field {
    State(Path),
    View(LitStr),
    Window(Expr),
    Title(LitStr),
    Init(Path),
    Subscriptions(Path),
}

impl Parse for Field {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        match ident.to_string().as_str() {
            "state" => Ok(Field::State(input.parse()?)),
            "view" => Ok(Field::View(input.parse()?)),
            "window" => Ok(Field::Window(input.parse()?)),
            "title" => Ok(Field::Title(input.parse()?)),
            "init" => Ok(Field::Init(input.parse()?)),
            "subscriptions" => Ok(Field::Subscriptions(input.parse()?)),
            other => Err(syn::Error::new(
                ident.span(),
                format!(
                    "libra::app!: unknown field '{other}' — expected one of: state, view, window, title, init, subscriptions"
                ),
            )),
        }
    }
}

#[derive(Default)]
struct AppSpec {
    state: Option<Path>,
    view: Option<LitStr>,
    window: Option<Expr>,
    title: Option<LitStr>,
    init: Option<Path>,
    subscriptions: Option<Path>,
}

impl Parse for AppSpec {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let fields = Punctuated::<Field, Token![,]>::parse_terminated(input)?;
        let mut spec = AppSpec::default();
        for f in fields {
            match f {
                Field::State(p) => spec.state = Some(p),
                Field::View(s) => spec.view = Some(s),
                Field::Window(e) => spec.window = Some(e),
                Field::Title(s) => spec.title = Some(s),
                Field::Init(p) => spec.init = Some(p),
                Field::Subscriptions(p) => spec.subscriptions = Some(p),
            }
        }
        Ok(spec)
    }
}

pub fn app(input: TokenStream) -> TokenStream {
    let spec = syn::parse_macro_input!(input as AppSpec);

    let state_ty = match spec.state {
        Some(p) => p,
        None => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                "libra::app!: requires a 'state' field naming the #[libra::state] type",
            )
            .to_compile_error()
            .into();
        }
    };
    let view_lit = match spec.view {
        Some(v) => v,
        None => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                "libra::app!: requires a 'view' field with the .lx path (relative to this file)",
            )
            .to_compile_error()
            .into();
        }
    };
    let title = spec.title.map(|t| quote! { #t }).unwrap_or_else(|| quote! { "Libra App" });
    let window: Expr = spec.window.unwrap_or_else(|| syn::parse_quote! { (800.0f32, 600.0f32) });
    let init_expr = match spec.init {
        Some(p) => quote! { #p() },
        None => quote! {
            (<#state_ty as ::std::default::Default>::default(), ::std::vec::Vec::new())
        },
    };
    // Optional: apps with ambient event sources beyond markup-declared
    // ones (§12.5's source elements don't yet support conditional
    // mounting — no <if> in the grammar, §26) still need a hand-written
    // subscriptions(&State) -> Vec<Sub<Action>> function. Omitted ⇒ the
    // App trait's own default (no subscriptions).
    let subscriptions_fn = spec.subscriptions.map(|p| {
        quote! {
            fn subscriptions(&self) -> ::std::vec::Vec<::libra_core::Sub<Action>> {
                #p(self)
            }
        }
    });

    quote! {
        pub const VIEW: &str = include_str!(#view_lit);

        impl ::libra_core::App for #state_ty {
            type Msg = Action;

            fn init() -> (Self, ::std::vec::Vec<::libra_core::Effect<Action>>) {
                #init_expr
            }

            fn update(&mut self, msg: Action) -> ::std::vec::Vec<::libra_core::Effect<Action>> {
                reduce(self, msg)
            }

            #subscriptions_fn
        }

        /// Generated by `libra::app!` (ARCHITECTURE.md §5.8): opens a
        /// native window running this app until closed. `main.rs`
        /// calls this and handles the `Result`.
        ///
        /// In debug builds, if the source `.lx` file is present on disk
        /// (i.e. running from the source tree, not an installed binary),
        /// it is watched so edits hot-reload with state preserved. The
        /// watch path is absolute (derived from `CARGO_MANIFEST_DIR`), so
        /// it works regardless of the current working directory; release
        /// builds and installed binaries never reference it.
        pub fn run() -> ::std::result::Result<(), ::std::boxed::Box<dyn ::std::error::Error>> {
            #[allow(unused_mut)]
            let mut opts = ::libra_platform::Options::new(#title, #window);
            #[cfg(debug_assertions)]
            {
                let dev_view = ::std::path::Path::new(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/",
                    #view_lit
                ));
                if dev_view.exists() {
                    opts = opts.watch(dev_view);
                }
            }
            ::libra_platform::run_with::<#state_ty>(VIEW, opts)
        }
    }
    .into()
}
