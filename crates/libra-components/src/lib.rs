//! libra-components — the component layer: ViewTree → taffy layout → a
//! laid-out tree carrying geometry + semantics that paints itself into a
//! DrawCmd list and hit-tests interactions back to handler names. Home of the
//! built-in catalog and the source elements.
//!
//! Module layout (PLAN-COMPONENTS): `node` (the shared vocabulary), `layout`
//! (ViewNode → LaidTree, dispatching per node to `catalog`), `paint` (LaidTree
//! → DrawCmds + hit testing, dispatching per item to `catalog`), and `catalog`
//! (one module per component — `build` + `paint`, tokens only). The public
//! surface stays flat via the re-exports below — downstream `use
//! libra_components::…` paths are unchanged.
//!
//! Architecture: ARCHITECTURE.md §7 (components), §8 (layout), §12.5
//! (declarative subscriptions).

mod catalog;
mod layout;
mod node;
mod paint;

pub use catalog::caret::caret_byte_at;
pub use layout::layout;
pub use node::*;
