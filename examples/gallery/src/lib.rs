//! libra gallery (§18 / Appendix C, via Storybook): one app rendering
//! every catalog widget × state × theme, snapshot-covered under both
//! built-in themes. Doubles as the PR visual-review surface: a widget
//! rendering change shows up as a gallery snapshot diff.

use libra_core::{App, Cmd, LibraReflect, SeriesHandle, SeriesId, SourceSpec};

pub const VIEW: &str = include_str!("../views/gallery.lx");
pub const WAVE: SeriesId = SeriesId(0);

#[derive(Debug, Clone, PartialEq, LibraReflect)]
pub struct Gallery {
    pub count: i64,
    pub draft: String,
    pub filled: String,
    pub theme: String,
    pub wave: SeriesHandle,
}

#[derive(Debug, Clone, PartialEq, LibraReflect)]
pub enum Msg {
    Noop,
    ToggleTheme,
    DraftChanged(String),
    FilledChanged(String),
}

impl App for Gallery {
    type Msg = Msg;

    fn init() -> (Self, Vec<Cmd<Msg>>) {
        (
            Gallery {
                count: 42,
                draft: String::new(),
                filled: "filled input".into(),
                theme: "libra-dark".into(),
                wave: SeriesHandle::new(WAVE, 2000.0),
            },
            // Live runs get a generator; tests feed the ring directly.
            vec![Cmd::IngestOpen {
                source: SourceSpec::Sine { rate_hz: 2000.0, freq_hz: 1.5, amp: 1.0, seed: 7 },
                series: WAVE,
            }],
        )
    }

    fn update(&mut self, msg: Msg) -> Vec<Cmd<Msg>> {
        match msg {
            Msg::Noop => vec![],
            Msg::ToggleTheme => {
                self.theme = if self.theme == "libra-dark" {
                    "libra-light".into()
                } else {
                    "libra-dark".into()
                };
                vec![Cmd::SetTheme { name: self.theme.clone() }]
            }
            Msg::DraftChanged(s) => {
                self.draft = s;
                vec![]
            }
            Msg::FilledChanged(s) => {
                self.filled = s;
                vec![]
            }
        }
    }
}
