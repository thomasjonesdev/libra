//! Todos — the §6.2 item-fields showcase: a list you interact with
//! per row. Each todo renders inside a `<for>` as a `<Button>` whose
//! `onPress={complete(t.id)}` / `onPress={remove(t.id)}` carries *that
//! row's* id as the action payload — loop-scoped field access
//! (PLAN-ITEM-FIELDS), not a Display-encoded line.
//!
//! The state layer is the point of the demo: `#[libra::state]` +
//! `#[libra::actions]` turn plain `&mut self` methods into the whole
//! `Action` enum + `reduce`. `items` is the source of truth;
//! `active`/`done`/`summary` are §6.3 display projections — and the
//! `#[project]`-marked method rebuilds them **automatically after every
//! action** (the generated `reduce` calls it; `app!`'s default init
//! projects the default state). Actions just mutate `items`; nothing
//! here remembers to refresh anything. Until `<if>` exists (§26), the
//! view keeps two filtered collections rather than one conditional list.

use libra::{actions, app, state};
use libra_core::LibraReflect;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, LibraReflect)]
pub struct Todo {
    pub id: u32,
    pub text: String,
    pub done: bool,
}

impl fmt::Display for Todo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Whole-value fallback (`key={t}`) — id is unique and stable
        // across a toggle.
        write!(f, "{}", self.id)
    }
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
    /// Display projections (§6.3) — never touched by actions; the
    /// `#[project]` hook below rebuilds them after every reduce.
    /// Structured `Todo` rows so markup can bind `{t.text}` / `t.id`.
    pub active: Vec<Todo>,
    pub done: Vec<Todo>,
    /// "N of M done", bound in the header.
    pub summary: String,
}

impl Todos {
    fn set_done(&mut self, id: &str, done: bool) {
        let Ok(id) = id.parse::<u32>() else {
            return;
        };
        if let Some(t) = self.items.iter_mut().find(|t| t.id == id) {
            t.done = done;
        }
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
    }

    /// Item-scoped: `onPress={complete(t.id)}` — `t.id` is the row id.
    pub fn complete(&mut self, id: String) {
        self.set_done(&id, true);
    }

    /// Item-scoped: `onPress={uncomplete(t.id)}`.
    pub fn uncomplete(&mut self, id: String) {
        self.set_done(&id, false);
    }

    /// Item-scoped: `onPress={remove(t.id)}` — drop that row.
    pub fn remove(&mut self, id: String) {
        let Ok(id) = id.parse::<u32>() else {
            return;
        };
        self.items.retain(|t| t.id != id);
    }

    /// Drop every completed todo.
    pub fn clear_completed(&mut self) {
        self.items.retain(|t| !t.done);
    }

    /// §6.3 display projections. Not an action — the generated `reduce`
    /// runs this after every action above (and `app!`'s default init
    /// runs it on the default state), so `active`/`done`/`summary` are
    /// always in sync with `items` without anyone remembering anything.
    #[project]
    fn project(&mut self) {
        self.active = self.items.iter().filter(|t| !t.done).cloned().collect();
        self.done = self.items.iter().filter(|t| t.done).cloned().collect();
        let total = self.items.len();
        let done = self.items.iter().filter(|t| t.done).count();
        self.summary = format!("{done} of {total} done");
    }
}

app! {
    state: Todos,
    view: "../views/todos.lx",
    window: (460.0, 520.0),
    title: "Libra Todos",
}
