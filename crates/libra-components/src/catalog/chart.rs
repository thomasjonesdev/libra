//! Chart / Sparkline — time-series line charts read from the data plane at
//! paint time (§7.1 Data, §14.2). `Chart` grows to fill width with panel
//! chrome; `Sparkline` is a fixed-size inline mini-chart, no chrome.

use crate::layout::BuildCx;
use crate::node::*;
use crate::paint::PaintCx;
use libra_core::SeriesHandle;
use libra_data::{m4_downsample, SeriesStore};
use libra_draw::{DrawCmd, Rect, Theme};
use taffy::prelude::*;

pub(crate) fn build_chart(
    cx: &mut BuildCx,
    handle: SeriesHandle,
    height: f32,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let id = cx.tree.new_leaf(Style {
        size: Size { width: auto(), height: length(height) },
        flex_grow: 1.0,
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::Chart { handle, panel: true },
        Semantics { role: Role::Chart, text: handle.to_string(), test_id: test_id.clone() },
        None,
    ));
    Ok(id)
}

pub(crate) fn build_sparkline(
    cx: &mut BuildCx,
    handle: SeriesHandle,
    width: f32,
    height: f32,
    test_id: &Option<String>,
) -> Result<NodeId, taffy::TaffyError> {
    let id = cx.tree.new_leaf(Style {
        size: Size { width: length(width), height: length(height) },
        flex_shrink: 0.0,
        ..Default::default()
    })?;
    cx.meta.push((
        id,
        ItemKind::Chart { handle, panel: false },
        Semantics { role: Role::Chart, text: handle.to_string(), test_id: test_id.clone() },
        None,
    ));
    Ok(id)
}

pub(crate) fn paint(cx: &mut PaintCx, item: &Item, handle: &SeriesHandle, panel: bool) {
    paint_chart(cx.cmds, item.rect, handle, panel, cx.store, cx.theme, cx.density);
}

/// Chart painting (§14.2 read path): M4 the ring's window to pixel
/// columns, autoscale y to the window's extremes, emit either column
/// strips (dense data — cheap opaque runs, the M4 paper's own rendering)
/// or a stroked polyline (sparse data — smooth). Pure function of
/// (ring contents, rect, density) — a fed tape renders the same bytes
/// every time at a given density.
///
/// `density` (Stage 6): column count scales with it so a Retina caller
/// gets one column per physical pixel; `col_w` is computed from
/// `inner.w` (logical) and the *unscaled* strip count, so after the
/// caller scales the resulting cmds by `density`, each column lands
/// exactly 1 physical pixel wide rather than 1/density.
#[allow(clippy::too_many_arguments)]
fn paint_chart(
    cmds: &mut Vec<DrawCmd>,
    rect: Rect,
    handle: &SeriesHandle,
    panel: bool,
    store: &SeriesStore,
    theme: &Theme,
    density: f32,
) {
    const PAD: f32 = 4.0;
    if panel {
        cmds.push(DrawCmd::RRect { rect, radius: theme.radius_md, color: theme.surface });
    }
    let inner = Rect::new(
        rect.x + PAD,
        rect.y + PAD,
        (rect.w - 2.0 * PAD).max(1.0),
        (rect.h - 2.0 * PAD).max(1.0),
    );

    let width_px = ((inner.w * density.max(0.01)).round() as u32).max(1);
    let cols = store.chart_columns(handle.id, handle.window_ms, width_px);
    if cols.samples < 2 {
        return; // no data: chrome only
    }

    let (mut vmin, mut vmax) = (f64::INFINITY, f64::NEG_INFINITY);
    for s in cols.strips.iter().flatten() {
        vmin = vmin.min(s.0);
        vmax = vmax.max(s.1);
    }
    let span = if vmax > vmin { vmax - vmin } else { 1.0 };
    let to_y = |v: f64| inner.y + (1.0 - ((v - vmin) / span) as f32) * inner.h;

    if cols.samples >= 2 * width_px as usize {
        // Dense: column min/max envelope as opaque strips (the M4
        // paper's own rendering — cheap by construction).
        let strips: Vec<Option<(f32, f32)>> = cols
            .strips
            .iter()
            .map(|o| o.map(|(lo, hi)| (to_y(hi), to_y(lo)))) // y inverts
            .collect();
        let col_w = inner.w / cols.strips.len().max(1) as f32;
        cmds.push(DrawCmd::Strips { x: inner.x, col_w, strips, color: theme.accent });
    } else {
        // Sparse: exact points, stroked smooth. (M4 passthrough keeps
        // them all; the snapshot copy is small by definition here.)
        let ring = store.ring(handle.id);
        let (samples, _) = ring.snapshot_from(cols.t_from);
        let pts = m4_downsample(&samples, cols.t_from, cols.t_to, width_px);
        if pts.len() < 2 {
            return;
        }
        let points: Vec<(f32, f32)> = pts
            .iter()
            .map(|p| {
                let fx = ((p.t_ms - cols.t_from) / (cols.t_to - cols.t_from)) as f32;
                (inner.x + fx * inner.w, to_y(p.v))
            })
            .collect();
        cmds.push(DrawCmd::Polyline { points, width: 1.25, color: theme.accent });
    }
}
