//! The Phase 3 reference app: text entry through the pure loop.
//! Typing runs the §10.1 editing core (in the shell/harness), each
//! keystroke dispatches `titleChanged(String)` — state stays the
//! single source of truth (controlled input) — and Save persists via
//! the effects runtime.

use libra::{actions, app, state};
use libra_core::{Effect, KvResult};

pub const TITLE_KEY: &str = "notes/title";

#[state]
pub struct Notes {
    pub title: String,
    pub status: String,
    /// Current theme name — mirrored to state so the toggle knows
    /// where it is (the shell owns the *applied* theme).
    pub theme: String,
}

impl Notes {
    pub fn init() -> (Notes, Vec<Effect<Action>>) {
        (
            Notes { title: String::new(), status: "unsaved".into(), theme: "libra-dark".into() },
            vec![Effect::kv_get(TITLE_KEY, Action::TitleLoaded)],
        )
    }
}

#[actions]
impl Notes {
    pub fn title_changed(&mut self, text: String) {
        self.title = text;
        self.status = "unsaved".into();
    }

    pub fn save(&mut self) -> Effect<Action> {
        Effect::kv_set_acked(TITLE_KEY, self.title.clone().into_bytes(), Action::Saved)
    }

    pub fn toggle_theme(&mut self) -> Effect<Action> {
        self.theme =
            if self.theme == "libra-dark" { "libra-light".into() } else { "libra-dark".into() };
        Effect::SetTheme { name: self.theme.clone() }
    }

    // Continuations (§5.2): effect-result payloads, not markup handlers.
    fn title_loaded(&mut self, bytes: Option<Vec<u8>>) {
        let Some(bytes) = bytes else { return };
        self.title = String::from_utf8_lossy(&bytes).into_owned();
        self.status = "loaded".into();
    }

    fn saved(&mut self, result: KvResult) {
        self.status = match result {
            KvResult::Ok => "saved".into(),
            KvResult::Err(e) => format!("save failed: {e}"),
        };
    }
}

app! {
    state: Notes,
    view: "../views/notes.lx",
    init: Notes::init,
    window: (520.0, 200.0),
    title: "Libra Notes",
}
