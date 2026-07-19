//! Source elements — `<Frames>` / `<Interval>` / `<Stream>` (§12.5). Build only:
//! each is an invisible, zero-layout leaf that declares a mount-scoped
//! subscription. `Role::Source` semantics are test-query-only (excluded from
//! the AccessKit projection); they paint nothing.

use crate::layout::BuildCx;
use crate::node::*;
use taffy::prelude::*;

fn zero_leaf(cx: &mut BuildCx) -> Result<NodeId, taffy::TaffyError> {
    cx.tree.new_leaf(Style {
        size: Size { width: length(0.0), height: length(0.0) },
        ..Default::default()
    })
}

pub(crate) fn build_frames(cx: &mut BuildCx, on_tick: &str) -> Result<NodeId, taffy::TaffyError> {
    let id = zero_leaf(cx)?;
    cx.meta.push((
        id,
        ItemKind::Source(SourceDecl::Frames { on_tick: on_tick.to_string() }),
        // `text: "frames"` is the source *kind*, matched by
        // `by::source("frames")` — not the handler name.
        Semantics { role: Role::Source, text: "frames".to_string(), test_id: None },
        None,
    ));
    Ok(id)
}

pub(crate) fn build_interval(
    cx: &mut BuildCx,
    every_ms: u64,
    on_tick: &str,
) -> Result<NodeId, taffy::TaffyError> {
    let id = zero_leaf(cx)?;
    cx.meta.push((
        id,
        ItemKind::Source(SourceDecl::Interval { every_ms, on_tick: on_tick.to_string() }),
        Semantics { role: Role::Source, text: "interval".to_string(), test_id: None },
        None,
    ));
    Ok(id)
}

pub(crate) fn build_stream(
    cx: &mut BuildCx,
    id_binding: &str,
    on_event: &str,
) -> Result<NodeId, taffy::TaffyError> {
    let id = zero_leaf(cx)?;
    cx.meta.push((
        id,
        ItemKind::Source(SourceDecl::Stream {
            id: id_binding.to_string(),
            on_event: on_event.to_string(),
        }),
        Semantics { role: Role::Source, text: "stream".to_string(), test_id: None },
        None,
    ));
    Ok(id)
}
