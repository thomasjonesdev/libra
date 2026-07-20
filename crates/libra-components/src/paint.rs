//! The paint pass (ARCHITECTURE.md §9.1): walk the `LaidTree`'s `Item`s and
//! emit the `DrawCmd` display list, plus hit testing.
//!
//! `paint_full` is a thin dispatcher: each arm delegates to the component's own
//! `catalog::<component>::paint`, which receives a [`PaintCx`] (the command
//! sink, measurer, data store, focus/hover input, theme, density). One file per
//! component — see PLAN-COMPONENTS.

use crate::catalog;
use crate::node::*;
use libra_data::SeriesStore;
use libra_draw::{DrawCmd, Theme};

/// Everything a component's `paint` needs. Passed by `&mut` so a component
/// pushes into `cmds`; the rest are read-only inputs to the pure paint.
pub(crate) struct PaintCx<'a> {
    pub cmds: &'a mut Vec<DrawCmd>,
    pub measurer: &'a dyn TextMeasurer,
    pub store: &'a SeriesStore,
    pub focus: Option<&'a InputFocus>,
    pub hover: Option<(f32, f32)>,
    pub theme: &'a Theme,
    pub density: f32,
}

impl LaidTree {
    /// Emit the display list. `store` is the data plane: Chart items
    /// read their ring here, at paint time (§14.2) — this is the one
    /// place samples meet the frame. `focus` paints the focused
    /// Input's ring, selection band, and caret.
    ///
    /// Delegates to `paint_at_density` at density 1.0 — the canonical
    /// frame (§9) snapshot tests assert against is density 1.0 by
    /// definition; every caller except the windowed shell (§21, off the
    /// determinism contract) wants exactly this.
    pub fn paint(
        &self,
        measurer: &dyn TextMeasurer,
        store: &SeriesStore,
        focus: Option<&InputFocus>,
        theme: &Theme,
    ) -> Vec<DrawCmd> {
        self.paint_full(measurer, store, focus, None, theme, 1.0)
    }

    /// `paint` with an explicit hover point (logical px). Hover is a
    /// *deterministic input* like focus, not ambient pointer state:
    /// the shell feeds its cursor, the harness feeds `None` by default
    /// (snapshots unchanged) or an explicit point for hover snapshots.
    /// Only `ListItem` reacts today (a `surface_raised` wash).
    pub fn paint_at_density(
        &self,
        measurer: &dyn TextMeasurer,
        store: &SeriesStore,
        focus: Option<&InputFocus>,
        theme: &Theme,
        density: f32,
    ) -> Vec<DrawCmd> {
        self.paint_full(measurer, store, focus, None, theme, density)
    }

    /// The full paint entry point: `density` is the PLAN-DASHBOARD-PERF
    /// Stage 6 presentation hint (charts compute M4 columns at
    /// `density`× resolution; the windowed shell passes its scale
    /// factor — never the canonical density-1.0 snapshot path), and
    /// `hover` is the optional pointer point (see `paint_at_density`'s
    /// doc for the determinism argument).
    pub fn paint_full(
        &self,
        measurer: &dyn TextMeasurer,
        store: &SeriesStore,
        focus: Option<&InputFocus>,
        hover: Option<(f32, f32)>,
        theme: &Theme,
        density: f32,
    ) -> Vec<DrawCmd> {
        let mut cmds = vec![DrawCmd::Clear(theme.bg)];
        {
            let mut cx = PaintCx { cmds: &mut cmds, measurer, store, focus, hover, theme, density };
            for item in &self.items {
                // PLAN-SCROLL: clip scrolled items to their viewport.
                if let Some(clip) = item.clip {
                    cx.cmds.push(DrawCmd::PushClip { rect: clip });
                }
                match &item.kind {
                    // Containers, scroll viewports, and mounted sources paint nothing.
                    ItemKind::Box | ItemKind::Scroll | ItemKind::Source(_) => {}
                    ItemKind::Separator => catalog::separator::paint(&mut cx, item),
                    ItemKind::ListItem { title, subtitle, meta, selected, px_md, px_sm, .. } => {
                        catalog::list_item::paint(&mut cx, item, title, subtitle, meta, *selected, *px_md, *px_sm)
                    }
                    ItemKind::Textarea { value, placeholder, px } => {
                        catalog::textarea::paint(&mut cx, item, value, placeholder, *px)
                    }
                    ItemKind::Chart { handle, panel } => {
                        catalog::chart::paint(&mut cx, item, handle, *panel)
                    }
                    ItemKind::Input { value, placeholder, px } => {
                        catalog::input::paint(&mut cx, item, value, placeholder, *px)
                    }
                    ItemKind::Text { content, px, style, color } => {
                        catalog::text::paint(&mut cx, item, content, *px, *style, *color)
                    }
                    ItemKind::Button { label, px, variant } => {
                        catalog::button::paint(&mut cx, item, label, *px, *variant)
                    }
                    ItemKind::Badge { label, tone, px } => {
                        catalog::badge::paint(&mut cx, item, label, *tone, *px)
                    }
                    ItemKind::Progress { value } => catalog::progress::paint(&mut cx, item, *value),
                    ItemKind::Scrim => catalog::dialog::paint_scrim(&mut cx, item.rect),
                    ItemKind::MenuBarrier => {}
                    ItemKind::Alert { tone, title, body, px_md, px_sm } => {
                        catalog::surface::paint_alert(&mut cx, item.rect, *tone, title, body, *px_md, *px_sm)
                    }
                    ItemKind::Surface { raised } => {
                        catalog::surface::paint_surface(&mut cx, item.rect, *raised)
                    }
                    ItemKind::Skeleton => catalog::surface::paint_skeleton(&mut cx, item.rect),
                    ItemKind::Avatar { initials, px } => {
                        catalog::surface::paint_avatar(&mut cx, item.rect, initials, *px)
                    }
                    ItemKind::SwitchCtl { checked } => {
                        catalog::toggle::paint_switch(&mut cx, item.rect, *checked)
                    }
                    ItemKind::CheckboxCtl { checked, label, px } => {
                        catalog::toggle::paint_checkbox(&mut cx, item.rect, *checked, label, *px)
                    }
                    ItemKind::RadioCtl { selected, label, px } => {
                        catalog::toggle::paint_radio(&mut cx, item.rect, *selected, label, *px)
                    }
                    ItemKind::MenuPanel => catalog::context_menu::paint_panel(&mut cx, item.rect),
                    ItemKind::Split { .. } => {}
                    ItemKind::SplitDivider { .. } => catalog::split::paint_divider(&mut cx, item.rect),
                    ItemKind::DialogPanel => catalog::dialog::paint_panel(&mut cx, item.rect),
                }
                if item.clip.is_some() {
                    cx.cmds.push(DrawCmd::PopClip);
                }
            }
        }
        cmds
    }

    /// Topmost interactive item containing the point — and, if it's inside a
    /// scroll viewport, only when the point is actually within that viewport
    /// (PLAN-SCROLL: a row scrolled out of view isn't hittable).
    ///
    /// `<Dialog>` items are the modal barrier: the panel swallows clicks on
    /// its body, and a scrim without `onDismiss` swallows everything — a
    /// point over either never falls through to the app beneath (a scrim
    /// *with* `onDismiss` is ordinary pressable, handled by the normal path).
    pub fn hit(&self, x: f32, y: f32) -> Option<&Item> {
        for i in self.items.iter().rev() {
            let contains = i.rect.contains(x, y) && i.clip.map_or(true, |c| c.contains(x, y));
            if !contains {
                continue;
            }
            if i.on_press.is_some() {
                return Some(i);
            }
            if matches!(
                i.kind,
                ItemKind::Scrim | ItemKind::DialogPanel | ItemKind::MenuBarrier | ItemKind::MenuPanel
            ) {
                return None; // modal barrier
            }
        }
        None
    }
}
