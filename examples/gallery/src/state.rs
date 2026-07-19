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
    pub is_textarea: bool,
    pub is_list_item: bool,
    pub is_dialog: bool,
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
            is_textarea: false,
            is_list_item: false,
            is_dialog: false,
            is_chart: false,
            is_sparkline: false,
            has_text_control: false,
            ctl_label: "Click me".into(),
            ctl_input: "editable".into(),
            ctl_body: "line one\nline two".into(),
            ctl_progress: 0.5,
            dialog_open: false,
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
