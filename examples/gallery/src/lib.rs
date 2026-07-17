//! libra gallery (§18 / Appendix C, via Storybook): one app rendering
//! every catalog component × state × theme, snapshot-covered under both
//! built-in themes. Doubles as the PR visual-review surface: a component
//! rendering change shows up as a gallery snapshot diff.

use libra::{actions, app, state};
use libra_core::{Effect, SeriesHandle, SeriesId, SourceSpec};

pub const WAVE: SeriesId = SeriesId(0);

#[state]
pub struct Gallery {
    pub count: i64,
    pub draft: String,
    pub filled: String,
    /// Multi-line cell for `<Textarea>`.
    pub body: String,
    pub theme: String,
    pub wave: SeriesHandle,
}

impl Gallery {
    pub fn init() -> (Gallery, Vec<Effect<Action>>) {
        (
            Gallery {
                count: 42,
                draft: String::new(),
                filled: "filled input".into(),
                body: "line one\nline two".into(),
                theme: "libra-dark".into(),
                wave: SeriesHandle::new(WAVE, 2000.0),
            },
            // Live runs get a generator; tests feed the ring directly.
            vec![Effect::IngestOpen {
                source: SourceSpec::Sine { rate_hz: 2000.0, freq_hz: 1.5, amp: 1.0, seed: 7 },
                series: WAVE,
            }],
        )
    }
}

#[actions]
impl Gallery {
    pub fn noop(&mut self) {}

    pub fn toggle_theme(&mut self) -> Effect<Action> {
        self.theme =
            if self.theme == "libra-dark" { "libra-light".into() } else { "libra-dark".into() };
        Effect::SetTheme { name: self.theme.clone() }
    }

    pub fn draft_changed(&mut self, s: String) {
        self.draft = s;
    }

    pub fn filled_changed(&mut self, s: String) {
        self.filled = s;
    }

    pub fn body_changed(&mut self, s: String) {
        self.body = s;
    }
}

app! {
    state: Gallery,
    view: "../views/gallery.lx",
    init: Gallery::init,
    window: (640.0, 360.0),
    title: "Libra Gallery",
}
