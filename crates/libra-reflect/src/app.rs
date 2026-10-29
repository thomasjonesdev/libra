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
//! - Emits `run()` + `options()`, not a literal `fn main()` — a
//!   function-like proc macro has no way to detect "is this crate's
//!   target a binary", so `main.rs` stays a one-line call to `run()`
//!   (`fn main() { myapp::run().unwrap() }`), same shape as today's
//!   examples but with the `impl App` and `pub const VIEW` gone. A
//!   `main.rs` that needs to customize the window (e.g. add a
//!   `theme_file`) builds on `options()` — the **single source** of the
//!   `title`/`window`/dev-watch — instead of re-declaring them, so the
//!   two can't drift (`examples/gallery` does this).
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
    Shortcuts(Path),
    Menu(Path),
    ContextMenu(Path),
    Tray(Path),
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
            "shortcuts" => Ok(Field::Shortcuts(input.parse()?)),
            "menu" => Ok(Field::Menu(input.parse()?)),
            "context_menu" => Ok(Field::ContextMenu(input.parse()?)),
            "tray" => Ok(Field::Tray(input.parse()?)),
            other => Err(syn::Error::new(
                ident.span(),
                format!(
                    "libra::app!: unknown field '{other}' — expected one of: state, view, window, title, init, subscriptions, shortcuts, menu, context_menu, tray"
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
    shortcuts: Option<Path>,
    menu: Option<Path>,
    context_menu: Option<Path>,
    tray: Option<Path>,
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
                Field::Shortcuts(p) => spec.shortcuts = Some(p),
                Field::Menu(p) => spec.menu = Some(p),
                Field::ContextMenu(p) => spec.context_menu = Some(p),
                Field::Tray(p) => spec.tray = Some(p),
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
    let title = spec
        .title
        .map(|t| quote! { #t })
        .unwrap_or_else(|| quote! { "Libra App" });
    let window: Expr = spec
        .window
        .unwrap_or_else(|| syn::parse_quote! { (800.0f32, 600.0f32) });
    let init_expr = match spec.init {
        Some(p) => quote! { #p() },
        // The projected default (§6.3): `Default::default()` run through
        // the block's `#[project]` hook (`__libra_project` is a no-op
        // when none is declared), so derived display fields are correct
        // from frame zero. Emitted by `#[libra::actions]`, which `app!`
        // already requires.
        None => quote! {
            {
                let mut state = <#state_ty as ::std::default::Default>::default();
                state.__libra_project();
                (state, ::std::vec::Vec::new())
            }
        },
    };
    // Optional: apps with ambient event sources beyond markup-declared
    // ones (§12.5's source elements don't yet support conditional
    // mounting — no <if> in the grammar, §26) still need a hand-written
    // subscriptions(&State) -> Vec<Sub<Action>> function. Omitted ⇒ the
    // App trait's own default (no subscriptions).
    let subscriptions_fn = spec.subscriptions.map(|p| {
        quote! {
            fn subscriptions(&self) -> ::std::vec::Vec<::libra::Sub<Action>> {
                #p(self)
            }
        }
    });
    // §19.2: static chord → action declarations, validated at startup
    // by the shell/harness via `build_shortcut_table`.
    let shortcuts_fn = spec.shortcuts.map(|p| {
        quote! {
            fn shortcuts() -> ::std::vec::Vec<::libra::Shortcut> {
                #p()
            }
        }
    });
    // §19.3: state-dependent native menubar. Omitted ⇒ empty (OS default).
    let menu_fn = spec.menu.map(|p| {
        quote! {
            fn menu(&self) -> ::libra::MenuDecl {
                #p(self)
            }
        }
    });
    let context_menu_fn = spec.context_menu.map(|p| {
        quote! {
            fn context_menu(&self) -> ::std::option::Option<::libra::MenuDecl> {
                #p(self)
            }
        }
    });
    let tray_fn = spec.tray.map(|p| {
        quote! {
            fn tray(&self) -> ::std::option::Option<::libra::TrayDecl> {
                #p(self)
            }
        }
    });

    quote! {
        pub const VIEW: &str = include_str!(#view_lit);

        #[cfg(all(not(debug_assertions), libra_has_view_codegen))]
        #[allow(clippy::all)]
        mod __libra_view_codegen {
            // PLAN-SDK S2: generated view.rs names these crates without a
            // leading `::` so this crate can alias them through the facade.
            // Conformance still resolves the real crates via the extern prelude.
            use ::libra::__private::components as libra_components;
            use ::libra as libra_core;
            include!(concat!(env!("OUT_DIR"), "/view.rs"));
        }

        #[cfg(all(not(debug_assertions), libra_has_view_codegen))]
        fn __libra_build_view(
            state: &#state_ty,
        ) -> ::libra::__private::components::ViewNode {
            __libra_view_codegen::build_view(state)
        }

        /// View source for the windowed shell (PLAN-PACKAGE P0): interpreter
        /// in debug; codegen in release when the crate's `build.rs` ran
        /// [`libra_build::compile_app`].
        pub fn view_driver() -> ::libra::ViewDriver<#state_ty> {
            #[cfg(all(not(debug_assertions), libra_has_view_codegen))]
            {
                ::libra::ViewDriver::with_codegen(VIEW, __libra_build_view)
            }
            #[cfg(not(all(not(debug_assertions), libra_has_view_codegen)))]
            {
                ::libra::ViewDriver::new(VIEW)
            }
        }

        impl ::libra::App for #state_ty {
            type Msg = Action;

            fn init() -> (Self, ::std::vec::Vec<::libra::Effect<Action>>) {
                #init_expr
            }

            fn update(&mut self, msg: Action) -> ::std::vec::Vec<::libra::Effect<Action>> {
                reduce(self, msg)
            }

            #subscriptions_fn

            #shortcuts_fn

            #menu_fn

            #context_menu_fn

            #tray_fn
        }

        /// Generated by `libra::app!` (ARCHITECTURE.md §5.8): the window
        /// `Options` this app declares — its `title` and `window` size, plus
        /// dev hot-reload watch in debug builds. The **single source** of the
        /// window size/title: a `main.rs` that needs to customize (e.g. add a
        /// `theme_file`) extends this rather than re-declaring, so the two can
        /// never drift.
        ///
        /// In debug builds, if the source `.lx` file is present on disk
        /// (i.e. running from the source tree, not an installed binary),
        /// it is watched so edits hot-reload with state preserved. The
        /// shell polls the view *directory* (PLAN-SDK S3) so a composed
        /// component `.lx` reload counts too. The watch path is absolute
        /// (derived from `CARGO_MANIFEST_DIR`), so it works regardless of
        /// the current working directory; release builds and installed
        /// binaries never reference it.
        pub fn options() -> ::libra::Options {
            #[allow(unused_mut)]
            let mut opts = ::libra::Options::new(#title, #window);
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
            opts
        }

        /// Generated by `libra::app!` (ARCHITECTURE.md §5.8): opens a
        /// native window running this app until closed, using [`options`].
        /// `main.rs` calls this and handles the `Result` — unless it needs
        /// to customize the options, in which case it builds on [`options`]
        /// and calls `::libra::run_with` itself.
        ///
        /// Debug builds use `run_rehydrating` (§18.2) so a `libra dev`
        /// Rust restart restores the last snapshot. The snapshot id is
        /// `app_id` when one is set, otherwise the crate name. Release
        /// builds stay on `run_with` — a packaged app does not resume a
        /// dev session.
        pub fn run() -> ::std::result::Result<(), ::std::boxed::Box<dyn ::std::error::Error>> {
            #[cfg(debug_assertions)]
            {
                let opts = options();
                let opts = if opts.app_id.is_none() {
                    opts.app_id(env!("CARGO_PKG_NAME"))
                } else {
                    opts
                };
                ::libra::run_rehydrating::<#state_ty>(view_driver(), opts)
            }
            #[cfg(not(debug_assertions))]
            {
                ::libra::run_with::<#state_ty>(view_driver(), options())
            }
        }
    }
    .into()
}
