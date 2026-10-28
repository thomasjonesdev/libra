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
                    // Containers, scroll viewports, mounted sources, and inline
                    // link hit targets paint nothing.
                    ItemKind::Box | ItemKind::Scroll | ItemKind::Source(_) | ItemKind::LinkTarget => {}
                    ItemKind::Separator => catalog::separator::paint(&mut cx, item),
                    ItemKind::ListItem { title, subtitle, meta, selected, px_md, px_sm, inset, .. } => {
                        catalog::list_item::paint(
                            &mut cx, item, title, subtitle, meta, *selected, *px_md, *px_sm, *inset,
                        )
                    }
                    ItemKind::Textarea { value, placeholder, px, markdown, .. } => {
                        catalog::textarea::paint(&mut cx, item, value, placeholder, *px, *markdown)
                    }
                    ItemKind::Chart { handle, panel } => {
                        catalog::chart::paint(&mut cx, item, handle, *panel)
                    }
                    ItemKind::Input { value, placeholder, px, variant, bold, .. } => {
                        catalog::input::paint(&mut cx, item, value, placeholder, *px, *variant, *bold)
                    }
                    ItemKind::Search { value, placeholder, px, .. } => {
                        catalog::search::paint(&mut cx, item, value, placeholder, *px)
                    }
                    ItemKind::Text { content, px, style, color, wrap, strike } => {
                        catalog::text::paint(
                            &mut cx, item, content, *px, *style, *color, *wrap, *strike,
                        )
                    }
                    ItemKind::Button { label, px, variant, icon } => {
                        catalog::button::paint(&mut cx, item, label, *px, *variant, *icon)
                    }
                    ItemKind::Icon { name, muted } => {
                        catalog::icon::paint(&mut cx, item, *name, *muted)
                    }
                    ItemKind::Badge { label, tone, px } => {
                        catalog::badge::paint(&mut cx, item, label, *tone, *px)
                    }
                    ItemKind::ChipBox { selected } => catalog::chip::paint_box(&mut cx, item, *selected),
                    ItemKind::ChipLabel { label, px, selected } => {
                        catalog::chip::paint_label(&mut cx, item, label, *px, *selected)
                    }
                    ItemKind::ChipRemove { selected } => {
                        catalog::chip::paint_remove(&mut cx, item, *selected)
                    }
                    ItemKind::TabsBox => catalog::tabs::paint_tabs(&mut cx, item),
                    ItemKind::ToolbarBox { vertical, compact, background } => {
                        catalog::toolbar::paint(
                            &mut cx,
                            item,
                            *vertical,
                            *compact,
                            background.as_deref(),
                        )
                    }
                    ItemKind::StatusBarBox => catalog::statusbar::paint(&mut cx, item),
                    ItemKind::TabBox { selected } => catalog::tabs::paint_box(&mut cx, item, *selected),
                    ItemKind::TabLabel { label, px, selected } => {
                        catalog::tabs::paint_label(&mut cx, item, label, *px, *selected)
                    }
                    ItemKind::TabClose { selected } => {
                        catalog::tabs::paint_close(&mut cx, item, *selected)
                    }
                    ItemKind::Progress { value } => catalog::progress::paint(&mut cx, item, *value),
                    ItemKind::Scrim => catalog::dialog::paint_scrim(&mut cx, item.rect),
                    ItemKind::MenuBarrier => {}
                    ItemKind::SliderCtl { value, .. } => {
                        catalog::slider::paint(&mut cx, item.rect, *value)
                    }
                    ItemKind::TooltipHost { .. } => {}
                    ItemKind::Alert { tone, title, body, px_md, px_sm } => {
                        catalog::surface::paint_alert(&mut cx, item.rect, *tone, title, body, *px_md, *px_sm)
                    }
                    ItemKind::ToastHost => {}
                    ItemKind::ToastBox { tone } => catalog::toast::paint_box(&mut cx, item, *tone),
                    ItemKind::ToastCopy { title, body, px_md, px_sm } => {
                        catalog::toast::paint_copy(&mut cx, item, title, body, *px_md, *px_sm)
                    }
                    ItemKind::ToastClose => catalog::toast::paint_close(&mut cx, item),
                    ItemKind::Surface { raised, flat } => {
                        catalog::surface::paint_surface(&mut cx, item.rect, *raised, *flat)
                    }
                    ItemKind::Skeleton => catalog::surface::paint_skeleton(&mut cx, item.rect),
                    ItemKind::Avatar { initials, px } => {
                        catalog::surface::paint_avatar(&mut cx, item.rect, initials, *px)
                    }
                    ItemKind::SwitchCtl { checked } => {
                        catalog::toggle::paint_switch(&mut cx, item.rect, *checked)
                    }
                    ItemKind::BreadcrumbBox
                    | ItemKind::PaginationBox
                    | ItemKind::StepperBox
                    | ItemKind::TimelineBox => {}
                    ItemKind::BreadcrumbLabel { label, current } => {
                        catalog::breadcrumb::paint_label(&mut cx, item, label, *current)
                    }
                    ItemKind::Chevron => catalog::breadcrumb::paint_chevron(&mut cx, item),
                    ItemKind::PageButton { label, active, enabled } => {
                        catalog::pagination::paint_button(&mut cx, item, label, *active, *enabled)
                    }
                    ItemKind::PageEllipsis => catalog::pagination::paint_ellipsis(&mut cx, item.rect),
                    ItemKind::StepperDot { label, status, px } => {
                        catalog::stepper::paint_dot(&mut cx, item, label, *status, *px)
                    }
                    ItemKind::Connector { vertical, filled } => {
                        if *vertical {
                            catalog::timeline::paint_connector(&mut cx, item, *filled)
                        } else {
                            catalog::stepper::paint_connector(&mut cx, item, *filled)
                        }
                    }
                    ItemKind::TimelineDot { title, meta, done, px_md, px_sm } => {
                        catalog::timeline::paint_item(&mut cx, item, title, meta, *done, *px_md, *px_sm)
                    }
                    ItemKind::CheckboxCtl { checked, label, px } => {
                        catalog::toggle::paint_checkbox(&mut cx, item.rect, *checked, label, *px)
                    }
                    ItemKind::RadioCtl { selected, label, px } => {
                        catalog::toggle::paint_radio(&mut cx, item.rect, *selected, label, *px)
                    }
                    ItemKind::MenuPanel => catalog::context_menu::paint_panel(&mut cx, item.rect),
                    ItemKind::Split { .. } => {}
                    ItemKind::SplitDivider { .. } => {
                        catalog::split::paint_divider(&mut cx, item.rect);
                    }
                    ItemKind::DialogPanel => catalog::dialog::paint_panel(&mut cx, item.rect),
                }
                if item.clip.is_some() {
                    cx.cmds.push(DrawCmd::PopClip);
                }
            }
            // Second phase: hovered tooltips top the z-order.
            if let Some((hx, hy)) = hover {
                for item in &self.items {
                    if let ItemKind::TooltipHost { text, position } = &item.kind {
                        let inside = item.rect.contains(hx, hy)
                            && item.clip.map_or(true, |c| c.contains(hx, hy));
                        if inside {
                            catalog::tooltip::paint_label(
                                &mut cx,
                                item.rect,
                                text,
                                *position,
                                (self.width, self.height),
                            );
                        }
                    }
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

    /// The pointer affordance at a point (§19) — the CSS `cursor:` analog,
    /// derived from the tree rather than authored in markup (see [`Cursor`]).
    ///
    /// Deliberately the same topmost-first walk as [`LaidTree::hit`], with the
    /// same clip and modal-barrier rules, so the shape can never promise an
    /// interaction the click path won't perform. It reports two extra targets
    /// that are pointer-interactive without living in the press slot: a
    /// `<Split>` divider gutter (drag → `ColResize`) and a `<Slider>` track
    /// (drag-to-set → `Pointer`).
    ///
    /// Static text is `Default`, not `Text`: Libra text isn't selectable, so
    /// an I-beam would advertise a gesture that does nothing.
    pub fn cursor_at(&self, x: f32, y: f32) -> Cursor {
        for i in self.items.iter().rev() {
            let contains = i.rect.contains(x, y) && i.clip.map_or(true, |c| c.contains(x, y));
            if !contains {
                continue;
            }
            match &i.kind {
                ItemKind::SplitDivider { .. } => return Cursor::ColResize,
                ItemKind::SliderCtl { .. } => return Cursor::Pointer,
                ItemKind::PageButton { enabled: false, .. } => return Cursor::NotAllowed,
                _ => {}
            }
            if i.on_press.is_some() {
                return match i.semantics.role {
                    Role::TextInput => Cursor::Text,
                    _ => Cursor::Pointer,
                };
            }
            if matches!(
                i.kind,
                ItemKind::Scrim | ItemKind::DialogPanel | ItemKind::MenuBarrier | ItemKind::MenuPanel
            ) {
                return Cursor::Default; // modal barrier: nothing beneath is reachable
            }
        }
        Cursor::Default
    }

    /// Empty `<Toolbar>` chrome — a frameless window may start a native
    /// drag here. Interactive children (buttons, tabs, inputs) and split
    /// gutters win first via [`hit`] / the divider walk in the shell.
    pub fn chrome_drag_at(&self, x: f32, y: f32) -> bool {
        self.items.iter().rev().any(|i| {
            matches!(i.kind, ItemKind::ToolbarBox { .. })
                && i.rect.contains(x, y)
                && i.clip.map_or(true, |c| c.contains(x, y))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libra_draw::{FontStyle, Rect};

    fn item(kind: ItemKind, rect: Rect, role: Role, on_press: Option<&str>) -> Item {
        Item {
            kind,
            rect,
            semantics: Semantics { role, text: String::new(), test_id: None },
            on_press: on_press.map(str::to_string),
            on_press_arg: None,
            clip: None,
            scroll: None,
            menu: None,
        }
    }

    fn tree(items: Vec<Item>) -> LaidTree {
        LaidTree { items, width: 200.0, height: 100.0 }
    }

    #[test]
    fn pressable_items_get_the_hand_and_inert_ones_do_not() {
        let laid = tree(vec![
            item(
                ItemKind::Text {
                    content: "label".into(),
                    px: 14.0,
                    style: FontStyle::Regular,
                    color: libra_draw::Color::rgb(255, 255, 255),
                    wrap: false,
                    strike: false,
                },
                Rect::new(0.0, 0.0, 100.0, 20.0),
                Role::Text,
                None,
            ),
            item(
                ItemKind::Button { label: String::new(), px: 14.0, variant: Variant::Primary, icon: None },
                Rect::new(0.0, 30.0, 100.0, 20.0),
                Role::Button,
                Some("press"),
            ),
            item(
                ItemKind::Input {
                    value: String::new(),
                    placeholder: String::new(),
                    px: 14.0,
                    on_submit: None,
                    variant: InputVariant::Field,
                    bold: false,
                },
                Rect::new(0.0, 60.0, 100.0, 20.0),
                Role::TextInput,
                Some("changed"),
            ),
        ]);
        assert_eq!(laid.cursor_at(10.0, 10.0), Cursor::Default);
        assert_eq!(laid.cursor_at(10.0, 40.0), Cursor::Pointer);
        assert_eq!(laid.cursor_at(10.0, 70.0), Cursor::Text);
        assert_eq!(laid.cursor_at(150.0, 90.0), Cursor::Default);
    }

    #[test]
    fn drag_targets_and_disabled_controls_report_their_own_shapes() {
        let laid = tree(vec![
            item(
                ItemKind::SplitDivider { split: "s".into() },
                Rect::new(100.0, 0.0, 6.0, 100.0),
                Role::Separator,
                None,
            ),
            item(
                ItemKind::SliderCtl { value: 0.5, on_change: "moved".into() },
                Rect::new(0.0, 0.0, 80.0, 20.0),
                Role::Container,
                None,
            ),
            item(
                ItemKind::PageButton { label: "‹".into(), active: false, enabled: false },
                Rect::new(0.0, 40.0, 20.0, 20.0),
                Role::Button,
                None,
            ),
        ]);
        assert_eq!(laid.cursor_at(102.0, 50.0), Cursor::ColResize);
        assert_eq!(laid.cursor_at(40.0, 10.0), Cursor::Pointer);
        assert_eq!(laid.cursor_at(10.0, 50.0), Cursor::NotAllowed);
    }

    /// The affordance must agree with `hit` on the two cases where a
    /// pressable rect is unreachable: scrolled out of its viewport, and
    /// behind a modal barrier.
    #[test]
    fn unreachable_presses_do_not_advertise_themselves() {
        let mut row = item(
            ItemKind::Button { label: String::new(), px: 14.0, variant: Variant::Primary, icon: None },
            Rect::new(0.0, 0.0, 100.0, 20.0),
            Role::Button,
            Some("press"),
        );
        row.clip = Some(Rect::new(0.0, 50.0, 100.0, 50.0));
        let scrolled = tree(vec![row]);
        assert!(scrolled.hit(10.0, 10.0).is_none());
        assert_eq!(scrolled.cursor_at(10.0, 10.0), Cursor::Default);

        let behind = tree(vec![
            item(
                ItemKind::Button { label: String::new(), px: 14.0, variant: Variant::Primary, icon: None },
                Rect::new(0.0, 0.0, 100.0, 20.0),
                Role::Button,
                Some("press"),
            ),
            item(ItemKind::Scrim, Rect::new(0.0, 0.0, 200.0, 100.0), Role::Container, None),
        ]);
        assert!(behind.hit(10.0, 10.0).is_none());
        assert_eq!(behind.cursor_at(10.0, 10.0), Cursor::Default);
    }

    #[test]
    fn empty_toolbar_chrome_is_a_window_drag_region() {
        let laid = tree(vec![
            item(
                ItemKind::ToolbarBox {
                    vertical: false,
                    compact: true,
                    background: None,
                },
                Rect::new(0.0, 0.0, 200.0, 36.0),
                Role::Container,
                None,
            ),
            item(
                ItemKind::Button {
                    label: String::new(),
                    px: 14.0,
                    variant: Variant::Ghost,
                    icon: None,
                },
                Rect::new(8.0, 4.0, 24.0, 28.0),
                Role::Button,
                Some("press"),
            ),
        ]);
        assert!(laid.chrome_drag_at(80.0, 16.0));
        assert!(laid.hit(80.0, 16.0).is_none());
        assert!(laid.hit(16.0, 16.0).is_some());
        assert!(!laid.chrome_drag_at(80.0, 80.0));
    }
}
