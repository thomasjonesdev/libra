//! The component catalog — **one module per built-in component** (§7.1).
//!
//! Each module owns its component end to end: `build` (measure + taffy +
//! semantics, taking a [`crate::layout::BuildCx`]) and, where it paints,
//! `paint` (DrawCmd emission, taking a [`crate::paint::PaintCx`]). Paint reads
//! **only theme tokens** (`cx.theme.*`) — never a raw hex/`Color::rgb` literal
//! (PLAN-TOKENS T3).
//!
//! **Adding a component** (PLAN-CATALOG's "one at a time"):
//! 1. Add a `ViewNode`/`ItemKind` variant in `node.rs` (closed grammar).
//! 2. Add the grammar in **both** engines + a `conformance/` fixture (§6.4).
//! 3. Add a `catalog/<component>.rs` module with `build` (+ `paint` if visible)
//!    and a dispatch arm in `layout::build` (and `paint_full` if visible).
//! 4. Add a gallery cell + snapshots; emit `Semantics`; stay on Appendix A.
//!
//! `text.rs` is the smallest build+paint example; `separator.rs` the smallest
//! paint; `source.rs` the smallest build-only (mount, no layout, no paint).

pub(crate) mod badge;
pub(crate) mod breadcrumb;
pub(crate) mod button;
pub(crate) mod chart;
pub(crate) mod chip;
pub(crate) mod container;
pub(crate) mod context_menu;
pub(crate) mod dialog;
pub(crate) mod drawer;
pub(crate) mod heading;
pub(crate) mod input;
pub(crate) mod list_item;
pub(crate) mod pagination;
pub(crate) mod progress;
pub(crate) mod scroll;
pub(crate) mod search;
pub(crate) mod separator;
pub(crate) mod slider;
pub(crate) mod source;
pub(crate) mod statusbar;
pub(crate) mod surface;
pub(crate) mod tabs;
pub(crate) mod toast;
pub(crate) mod toggle;
pub(crate) mod toolbar;
pub(crate) mod tooltip;
pub(crate) mod split;
pub(crate) mod spacer;
pub(crate) mod stepper;
pub(crate) mod text;
pub(crate) mod timeline;
pub(crate) mod textarea;
