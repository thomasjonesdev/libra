//! The actions (§5.9 module layout): the crate's one `#[libra::actions]` block
//! — sidebar selection, control edits, theme toggle, and the `#[project]` hook
//! that rebuilds the sidebar rows + per-component `is_*` flags from `selected`.
//!
//! Architecture: ARCHITECTURE.md §5.9 (module layout), §6.3 (projections).

use libra_core::Effect;

use crate::model::{name_of, rows, CONTROLS, DATA, DISPLAY, LAYOUT};
use crate::state::Gallery;

#[libra::actions]
impl Gallery {
    /// Sidebar click — `onPress={select(c.id)}` carries the component id.
    pub fn select(&mut self, id: String) {
        self.selected = id;
    }

    pub fn label_changed(&mut self, s: String) {
        self.ctl_label = s;
    }

    pub fn input_changed(&mut self, s: String) {
        self.ctl_input = s;
    }

    pub fn body_changed(&mut self, s: String) {
        self.ctl_body = s;
    }

    pub fn toggle_theme(&mut self) -> Effect<Action> {
        self.theme =
            if self.theme == "libra-dark" { "libra-light".into() } else { "libra-dark".into() };
        Effect::SetTheme { name: self.theme.clone() }
    }

    /// Playground press target and variant demos — nothing to record.
    pub fn noop(&mut self) {}

    pub fn open_dialog(&mut self) {
        self.dialog_open = true;
    }

    pub fn close_dialog(&mut self) {
        self.dialog_open = false;
    }

    /// `onMenu={openMenu}` on the ListItem cell (M7).
    pub fn open_menu(&mut self) {
        self.menu_open = true;
    }

    pub fn menu_close(&mut self) {
        self.menu_open = false;
    }

    pub fn toggle_switch(&mut self) {
        self.switch_on = !self.switch_on;
    }

    pub fn toggle_check(&mut self) {
        self.check_on = !self.check_on;
    }

    pub fn pick_radio_a(&mut self) {
        self.radio_pick = "a".into();
    }

    pub fn pick_radio_b(&mut self) {
        self.radio_pick = "b".into();
    }

    pub fn open_drawer(&mut self) {
        self.drawer_open = true;
    }

    pub fn close_drawer(&mut self) {
        self.drawer_open = false;
    }

    /// `<Split onResize>` echo (M5 controlled shape).
    pub fn split_changed(&mut self, fraction: String) {
        if let Ok(f) = fraction.parse::<f32>() {
            self.ctl_split = f.clamp(0.05, 0.95);
        }
    }

    /// Progress numeric-binding demo: nudge `ctl_progress` in clean 0.1 steps
    /// (rounded so the bound value's Display stays tidy).
    pub fn progress_up(&mut self) {
        self.ctl_progress = (((self.ctl_progress + 0.1) * 10.0).round() / 10.0).min(1.0);
    }

    pub fn progress_down(&mut self) {
        self.ctl_progress = (((self.ctl_progress - 0.1) * 10.0).round() / 10.0).max(0.0);
    }

    /// §6.3: rebuilt after every action, so the sidebar highlight and the
    /// selected canvas block always match `selected`.
    #[project]
    fn project(&mut self) {
        self.layout_rows = rows(LAYOUT, &self.selected);
        self.display_rows = rows(DISPLAY, &self.selected);
        self.control_rows = rows(CONTROLS, &self.selected);
        self.data_rows = rows(DATA, &self.selected);
        self.selected_name = name_of(&self.selected);

        let s = self.selected.as_str();
        self.is_column = s == "column";
        self.is_row = s == "row";
        self.is_spacer = s == "spacer";
        self.is_separator = s == "separator";
        self.is_text = s == "text";
        self.is_heading = s == "heading";
        self.is_badge = s == "badge";
        self.is_progress = s == "progress";
        self.is_button = s == "button";
        self.is_input = s == "input";
        self.is_textarea = s == "textarea";
        self.is_list_item = s == "list_item";
        self.is_chart = s == "chart";
        self.is_sparkline = s == "sparkline";
        self.is_dialog = s == "dialog";
        self.is_split = s == "split";
        self.is_drawer = s == "drawer";
        self.is_card = s == "card";
        self.is_panel = s == "panel";
        self.is_alert = s == "alert";
        self.is_skeleton = s == "skeleton";
        self.is_avatar = s == "avatar";
        self.is_switch = s == "switch";
        self.is_checkbox = s == "checkbox";
        self.is_radio = s == "radio";
        self.radio_a = self.radio_pick == "a";
        self.radio_b = self.radio_pick == "b";
        self.has_text_control = self.is_text || self.is_heading || self.is_badge || self.is_button;
    }
}
