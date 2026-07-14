//! The Phase 3 reference app: text entry through the pure loop.
//! Typing runs the §10.1 editing core (in the shell/harness), each
//! keystroke dispatches `TitleChanged(String)` — the model stays the
//! single source of truth (controlled input) — and Save persists via
//! the effects runtime.

use libra_core::{App, Cmd, KvResult, LibraReflect};

pub const VIEW: &str = include_str!("../views/notes.lx");
pub const TITLE_KEY: &str = "notes/title";

#[derive(Debug, Clone, PartialEq, LibraReflect)]
pub struct Notes {
    pub title: String,
    pub status: String,
    /// Current theme name — mirrored to the model so the toggle knows
    /// where it is (the shell owns the *applied* theme).
    pub theme: String,
}

#[derive(Debug, Clone, PartialEq, LibraReflect)]
pub enum Msg {
    TitleChanged(String),
    Save,
    ToggleTheme,
    TitleLoaded(Option<Vec<u8>>),
    Saved(KvResult),
}

impl App for Notes {
    type Msg = Msg;

    fn init() -> (Self, Vec<Cmd<Msg>>) {
        (
            Notes { title: String::new(), status: "unsaved".into(), theme: "libra-dark".into() },
            vec![Cmd::kv_get(TITLE_KEY, Msg::TitleLoaded)],
        )
    }

    fn update(&mut self, msg: Msg) -> Vec<Cmd<Msg>> {
        match msg {
            Msg::TitleChanged(text) => {
                self.title = text;
                self.status = "unsaved".into();
                vec![]
            }
            Msg::Save => {
                vec![Cmd::kv_set_acked(TITLE_KEY, self.title.clone().into_bytes(), Msg::Saved)]
            }
            Msg::ToggleTheme => {
                self.theme = if self.theme == "libra-dark" {
                    "libra-light".into()
                } else {
                    "libra-dark".into()
                };
                vec![Cmd::SetTheme { name: self.theme.clone() }]
            }
            Msg::TitleLoaded(Some(bytes)) => {
                self.title = String::from_utf8_lossy(&bytes).into_owned();
                self.status = "loaded".into();
                vec![]
            }
            Msg::TitleLoaded(None) => vec![],
            Msg::Saved(KvResult::Ok) => {
                self.status = "saved".into();
                vec![]
            }
            Msg::Saved(KvResult::Err(e)) => {
                self.status = format!("save failed: {e}");
                vec![]
            }
        }
    }
}
