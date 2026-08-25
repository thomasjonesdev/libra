//! The State (§5.9 module layout): the one `#[libra::state]` struct + init.
//! `selected` is the source of truth; the sidebar rows and the per-component
//! `is_*` canvas flags are §6.3 projections rebuilt by the `#[project]` hook in
//! `actions.rs`.
//!
//! Architecture: ARCHITECTURE.md §5.9 (module layout), §6.3 (projections).

use libra_core::{Effect, SeriesHandle, SeriesId, SourceSpec};

use crate::model::{ComponentRow, DEFAULT};
use crate::Action;

pub const WAVE: SeriesId = SeriesId(0);

#[libra::state]
pub struct Gallery {
    pub theme: String,
    /// Selected component id — the source of truth for everything below.
    pub selected: String,
    pub selected_name: String,

    // --- sidebar rows (projected per group) ---
    pub layout_rows: Vec<ComponentRow>,
    pub display_rows: Vec<ComponentRow>,
    pub control_rows: Vec<ComponentRow>,
    pub data_rows: Vec<ComponentRow>,

    // --- which canvas block shows (projected from `selected`) ---
    pub is_column: bool,
    pub is_row: bool,
    pub is_spacer: bool,
    pub is_separator: bool,
    pub is_text: bool,
    pub is_heading: bool,
    pub is_badge: bool,
    pub is_progress: bool,
    pub is_button: bool,
    pub is_input: bool,
    pub is_search: bool,
    pub is_chip: bool,
    pub is_textarea: bool,
    pub is_list_item: bool,
    pub is_dialog: bool,
    pub is_drawer: bool,
    pub is_card: bool,
    pub is_panel: bool,
    pub is_alert: bool,
    pub is_skeleton: bool,
    pub is_avatar: bool,
    pub is_switch: bool,
    pub is_checkbox: bool,
    pub is_radio: bool,
    pub is_slider: bool,
    pub is_tooltip: bool,
    pub is_breadcrumb: bool,
    pub is_pagination: bool,
    pub is_stepper: bool,
    pub is_timeline: bool,
    pub is_tabs: bool,
    pub is_toast: bool,
    pub is_split: bool,
    pub is_chart: bool,
    pub is_sparkline: bool,
    /// The selected component exposes a text control (label playground).
    pub has_text_control: bool,

    // --- controls (shared playground state) ---
    pub ctl_label: String,
    pub ctl_input: String,
    pub ctl_body: String,
    /// Numeric-binding demo: `<Progress value={ctl_progress}>` reads this f32
    /// live (the +/− buttons move it) — proof that numeric attrs bind (§26 #8).
    pub ctl_progress: f32,
    /// Dialog playground: the modal is mounted while this is set (M6).
    pub dialog_open: bool,
    /// ContextMenu playground: mounted while set (M7).
    pub menu_open: bool,
    /// Drawer playground.
    pub drawer_open: bool,
    /// Toggle playground (model-owned controls).
    pub switch_on: bool,
    pub check_on: bool,
    /// Radio playground: which option id is picked.
    pub radio_pick: String,
    pub radio_a: bool,
    pub radio_b: bool,
    /// Slider playground value (model-owned).
    pub ctl_slider: f32,
    /// Nav playground: current page / step (shared 0-based index).
    pub ctl_page: u32,
    /// Split playground fraction — echoed by `splitChanged` (M5).
    pub ctl_split: f32,
    /// Search playground: set when Enter fires `onSubmit`.
    pub search_submitted: bool,
    /// Chip playground (model-owned selected + dismiss).
    pub chip_on: bool,
    pub chip_dismissed: bool,
    /// Tabs playground (model-owned selected + closable scratch).
    pub tab_pick: String,
    pub tab_editor: bool,
    pub tab_preview: bool,
    pub tab_body: String,
    pub tab_dismissed: bool,
    /// Toast playground (overlay dismiss).
    pub toast_dismissed: bool,

    // --- data plane: the live series Chart/Sparkline read at paint time ---
    pub wave: SeriesHandle,
}

impl Gallery {
    pub fn init() -> (Gallery, Vec<Effect<Action>>) {
        let mut state = Gallery {
            theme: "libra-dark".into(),
            selected: DEFAULT.into(),
            selected_name: String::new(),
            layout_rows: Vec::new(),
            display_rows: Vec::new(),
            control_rows: Vec::new(),
            data_rows: Vec::new(),
            is_column: false,
            is_row: false,
            is_spacer: false,
            is_separator: false,
            is_text: false,
            is_heading: false,
            is_badge: false,
            is_progress: false,
            is_button: false,
            is_input: false,
            is_search: false,
            is_chip: false,
            is_textarea: false,
            is_list_item: false,
            is_dialog: false,
            is_drawer: false,
            is_card: false,
            is_panel: false,
            is_alert: false,
            is_skeleton: false,
            is_avatar: false,
            is_switch: false,
            is_checkbox: false,
            is_radio: false,
            is_slider: false,
            is_tooltip: false,
            is_breadcrumb: false,
            is_pagination: false,
            is_stepper: false,
            is_timeline: false,
            is_tabs: false,
            is_toast: false,
            is_split: false,
            is_chart: false,
            is_sparkline: false,
            has_text_control: false,
            ctl_label: "Click me".into(),
            ctl_input: "editable".into(),
            ctl_body: "line one\nline two".into(),
            ctl_progress: 0.5,
            dialog_open: false,
            menu_open: false,
            drawer_open: false,
            switch_on: true,
            check_on: true,
            radio_pick: "a".into(),
            radio_a: true,
            radio_b: false,
            ctl_slider: 0.5,
            ctl_page: 1,
            ctl_split: 0.5,
            search_submitted: false,
            chip_on: true,
            chip_dismissed: false,
            tab_pick: "editor".into(),
            tab_editor: true,
            tab_preview: false,
            tab_body: String::new(),
            tab_dismissed: false,
            toast_dismissed: false,
            wave: SeriesHandle::new(WAVE, 2000.0),
        };
        // Fill the §6.3 projections for frame zero (custom `init:` skips the
        // macro's default project).
        state.__libra_project();
        (
            state,
            // The live wave drives the Chart/Sparkline canvases; a live data
            // plane repaints the view on its own (§12.5) — no `<Frames>`.
            vec![Effect::IngestOpen {
                source: SourceSpec::Sine { rate_hz: 2000.0, freq_hz: 1.5, amp: 1.0, seed: 7 },
                series: WAVE,
            }],
        )
    }
}
