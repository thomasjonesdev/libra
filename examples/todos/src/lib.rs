//! Todos — the §6.2 item-scoped-handler showcase: a list you interact
//! with per row. Each todo renders inside a `<for>` as a `<Button>`
//! whose `onPress={complete(t)}` / `onPress={remove(t)}` carries *that
//! row's* value as the action payload — the loop binding becomes the
//! `String` argument the reducer receives.
//!
//! The state layer is the point of the demo: `#[libra::state]` +
//! `#[libra::actions]` turn plain `&mut self` methods into the whole
//! `Action` enum + `reduce`. `items` is the source of truth;
//! `active`/`done`/`summary` are §6.3 display projections rebuilt after
//! every mutation, so the view binds simple `Vec<String>`/`String`
//! fields and never has to compute anything itself (the closed grammar
//! can't — §6.2). Identity rides in each row's `"<id>. <text>"` line:
//! unique (the id is monotonic), human-readable, and stable across a
//! toggle, so it doubles as the key *and* the action payload.

use libra::{actions, app, state};
use libra_core::Effect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Todo {
    pub id: u32,
    pub text: String,
    pub done: bool,
}

#[derive(Default)]
#[state]
pub struct Todos {
    /// The new-todo input text (controlled: state is the source of truth).
    pub draft: String,
    /// Source of truth. Not bound directly — the view iterates the
    /// projections below — so it stays out of reflection.
    #[libra(skip)]
    pub items: Vec<Todo>,
    /// Monotonic id source; excluded from reflection (not display data).
    #[libra(skip)]
    pub next_id: u32,
    /// Display projections (§6.3), rebuilt by `reproject` after every
    /// mutation. Each entry is a row line — see `line`.
    pub active: Vec<String>,
    pub done: Vec<String>,
    /// "N of M done", bound in the header.
    pub summary: String,
}

impl Todos {
    pub fn init() -> (Todos, Vec<Effect<Action>>) {
        let mut app = Todos::default();
        app.reproject();
        (app, Vec::new())
    }

    /// A row's line — its identity, key, and action payload in one.
    /// `"3. Buy milk"`: the id keeps it unique and stable across a
    /// toggle; the text keeps it readable.
    fn line(t: &Todo) -> String {
        format!("{}. {}", t.id, t.text)
    }

    /// Rebuild the display projections from `items`.
    fn reproject(&mut self) {
        self.active = self.items.iter().filter(|t| !t.done).map(Self::line).collect();
        self.done = self.items.iter().filter(|t| t.done).map(Self::line).collect();
        let total = self.items.len();
        let done = self.items.iter().filter(|t| t.done).count();
        self.summary = format!("{done} of {total} done");
    }

    fn set_done(&mut self, line: &str, done: bool) {
        if let Some(t) = self.items.iter_mut().find(|t| Self::line(t) == line) {
            t.done = done;
        }
        self.reproject();
    }
}

#[actions]
impl Todos {
    /// Input `onChange` — the control supplies the new text.
    pub fn draft_changed(&mut self, text: String) {
        self.draft = text;
    }

    /// Add the drafted todo (Enter or the Add button). Empty/whitespace
    /// drafts are a no-op.
    pub fn add(&mut self) {
        let text = self.draft.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.items.push(Todo { id: self.next_id, text, done: false });
        self.next_id += 1;
        self.draft.clear();
        self.reproject();
    }

    /// Item-scoped: `onPress={complete(t)}` — `t` is the row's line.
    pub fn complete(&mut self, line: String) {
        self.set_done(&line, true);
    }

    /// Item-scoped: `onPress={uncomplete(t)}`.
    pub fn uncomplete(&mut self, line: String) {
        self.set_done(&line, false);
    }

    /// Item-scoped: `onPress={remove(t)}` — drop that row.
    pub fn remove(&mut self, line: String) {
        self.items.retain(|t| Self::line(t) != line);
        self.reproject();
    }

    /// Drop every completed todo.
    pub fn clear_completed(&mut self) {
        self.items.retain(|t| !t.done);
        self.reproject();
    }
}

app! {
    state: Todos,
    view: "../views/todos.lx",
    init: Todos::init,
    window: (460.0, 520.0),
    title: "Libra Todos",
}
