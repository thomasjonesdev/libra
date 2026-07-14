//! libra-text — the text-entry editing core (ARCHITECTURE.md §10.1).
//!
//! The heart of the spec's promise: *the editing core is a pure state
//! machine (`EditState × EditOp → EditState`) exercised without
//! rendering*. No clocks, no platform, no fonts — every component,
//! harness verb, and (later) IME path drives this same machine, and
//! Unit tests cover it exhaustively.
//!
//! Phase 3 scope: single-line plain text, char-boundary navigation
//! (grapheme clusters arrive with the parley/swash stack), per-field
//! undo/redo grouped at word boundaries. Multi-line (sticky column,
//! line ops), IME composition, and the rope backing for megabyte
//! pastes are specced (§10.1) and land with Textarea.

/// Caret + selection over a single-line string. `caret` and `anchor`
/// are byte indices on char boundaries; `caret == anchor` means no
/// selection. All mutation goes through [`EditState::apply`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EditState {
    text: String,
    caret: usize,
    anchor: usize,
    undo: Vec<(String, usize, usize)>,
    redo: Vec<(String, usize, usize)>,
    /// Last op class, for undo grouping (word-boundary snapshots).
    last_kind: OpKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum OpKind {
    #[default]
    Other,
    /// Run of inserts inside one word (groups as one undo step).
    InsertWord,
    /// Run of backspaces/deletes (groups as one undo step).
    DeleteRun,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditOp {
    Insert(char),
    InsertStr(String),
    Backspace,
    Delete,
    /// `true` extends the selection (Shift held).
    Left(bool),
    Right(bool),
    WordLeft(bool),
    WordRight(bool),
    Home(bool),
    End(bool),
    SelectAll,
    /// Place the caret at a byte offset (clamped to a char boundary).
    SetCaret { byte: usize, select: bool },
    Undo,
    Redo,
}

impl EditState {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let caret = text.len();
        EditState { caret, anchor: caret, text, ..Default::default() }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn caret(&self) -> usize {
        self.caret
    }
    /// Selected byte range (empty when caret == anchor).
    pub fn selection(&self) -> std::ops::Range<usize> {
        self.caret.min(self.anchor)..self.caret.max(self.anchor)
    }
    pub fn selected_text(&self) -> &str {
        &self.text[self.selection()]
    }

    /// The state machine. Every editing affordance — keyboard, harness
    /// verbs, later IME commit — reduces to these transitions.
    pub fn apply(&mut self, op: EditOp) {
        match op {
            EditOp::Insert(ch) => {
                self.snapshot_if(!(self.last_kind == OpKind::InsertWord && !boundary_char(ch)));
                self.last_kind = if boundary_char(ch) { OpKind::Other } else { OpKind::InsertWord };
                self.replace_selection(&ch.to_string());
            }
            EditOp::InsertStr(s) => {
                self.snapshot_if(true);
                self.last_kind = OpKind::Other;
                self.replace_selection(&s);
            }
            EditOp::Backspace => {
                if self.selection().is_empty() && self.caret == 0 {
                    return;
                }
                self.snapshot_if(self.last_kind != OpKind::DeleteRun);
                self.last_kind = OpKind::DeleteRun;
                if self.selection().is_empty() {
                    let prev = prev_boundary(&self.text, self.caret);
                    self.anchor = prev;
                }
                self.replace_selection("");
            }
            EditOp::Delete => {
                if self.selection().is_empty() && self.caret == self.text.len() {
                    return;
                }
                self.snapshot_if(self.last_kind != OpKind::DeleteRun);
                self.last_kind = OpKind::DeleteRun;
                if self.selection().is_empty() {
                    self.anchor = next_boundary(&self.text, self.caret);
                }
                self.replace_selection("");
            }
            EditOp::Left(select) => {
                self.last_kind = OpKind::Other;
                let to = if !select && !self.selection().is_empty() {
                    self.selection().start // collapse to selection start
                } else {
                    prev_boundary(&self.text, self.caret)
                };
                self.move_caret(to, select);
            }
            EditOp::Right(select) => {
                self.last_kind = OpKind::Other;
                let to = if !select && !self.selection().is_empty() {
                    self.selection().end
                } else {
                    next_boundary(&self.text, self.caret)
                };
                self.move_caret(to, select);
            }
            EditOp::WordLeft(select) => {
                self.last_kind = OpKind::Other;
                let to = prev_word(&self.text, self.caret);
                self.move_caret(to, select);
            }
            EditOp::WordRight(select) => {
                self.last_kind = OpKind::Other;
                let to = next_word(&self.text, self.caret);
                self.move_caret(to, select);
            }
            EditOp::Home(select) => {
                self.last_kind = OpKind::Other;
                self.move_caret(0, select);
            }
            EditOp::End(select) => {
                self.last_kind = OpKind::Other;
                self.move_caret(self.text.len(), select);
            }
            EditOp::SelectAll => {
                self.last_kind = OpKind::Other;
                self.anchor = 0;
                self.caret = self.text.len();
            }
            EditOp::SetCaret { byte, select } => {
                self.last_kind = OpKind::Other;
                let to = clamp_boundary(&self.text, byte);
                self.move_caret(to, select);
            }
            EditOp::Undo => {
                self.last_kind = OpKind::Other;
                if let Some((text, caret, anchor)) = self.undo.pop() {
                    self.redo.push((self.text.clone(), self.caret, self.anchor));
                    self.text = text;
                    self.caret = caret;
                    self.anchor = anchor;
                }
            }
            EditOp::Redo => {
                self.last_kind = OpKind::Other;
                if let Some((text, caret, anchor)) = self.redo.pop() {
                    self.undo.push((self.text.clone(), self.caret, self.anchor));
                    self.text = text;
                    self.caret = caret;
                    self.anchor = anchor;
                }
            }
        }
    }

    fn snapshot_if(&mut self, cond: bool) {
        if cond {
            self.undo.push((self.text.clone(), self.caret, self.anchor));
            self.redo.clear();
        }
    }

    fn replace_selection(&mut self, insert: &str) {
        let sel = self.selection();
        self.text.replace_range(sel.clone(), insert);
        self.caret = sel.start + insert.len();
        self.anchor = self.caret;
    }

    fn move_caret(&mut self, to: usize, select: bool) {
        self.caret = to;
        if !select {
            self.anchor = to;
        }
    }
}

fn boundary_char(ch: char) -> bool {
    ch.is_whitespace()
}

fn clamp_boundary(text: &str, byte: usize) -> usize {
    let mut b = byte.min(text.len());
    while b > 0 && !text.is_char_boundary(b) {
        b -= 1;
    }
    b
}

fn prev_boundary(text: &str, from: usize) -> usize {
    text[..from].char_indices().next_back().map_or(0, |(i, _)| i)
}

fn next_boundary(text: &str, from: usize) -> usize {
    text[from..].chars().next().map_or(from, |c| from + c.len_utf8())
}

fn is_word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Caret target for Alt/Ctrl+Left: start of the previous word.
fn prev_word(text: &str, from: usize) -> usize {
    let mut chars: Vec<(usize, char)> = text[..from].char_indices().collect();
    // Skip non-word chars, then the word itself.
    while let Some(&(_, c)) = chars.last() {
        if is_word(c) {
            break;
        }
        chars.pop();
    }
    while let Some(&(i, c)) = chars.last() {
        if !is_word(c) {
            return chars.last().map_or(0, |_| i + c.len_utf8());
        }
        chars.pop();
        if chars.is_empty() {
            return 0;
        }
    }
    chars.last().map_or(0, |&(i, c)| i + c.len_utf8())
}

/// Caret target for Alt/Ctrl+Right: end of the next word.
fn next_word(text: &str, from: usize) -> usize {
    let mut it = text[from..].char_indices().peekable();
    let mut pos = from;
    // Skip non-word chars.
    while let Some(&(i, c)) = it.peek() {
        if is_word(c) {
            break;
        }
        it.next();
        pos = from + i + c.len_utf8();
    }
    // Consume the word.
    while let Some(&(i, c)) = it.peek() {
        if !is_word(c) {
            break;
        }
        it.next();
        pos = from + i + c.len_utf8();
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(s: &str) -> EditState {
        let mut e = EditState::default();
        for ch in s.chars() {
            e.apply(EditOp::Insert(ch));
        }
        e
    }

    #[test]
    fn typing_appends_at_caret() {
        let e = typed("hello");
        assert_eq!(e.text(), "hello");
        assert_eq!(e.caret(), 5);
    }

    #[test]
    fn selection_replace_and_collapse() {
        let mut e = typed("hello world");
        e.apply(EditOp::Home(false));
        e.apply(EditOp::WordRight(true)); // select "hello"
        assert_eq!(e.selected_text(), "hello");
        e.apply(EditOp::Insert('H'));
        assert_eq!(e.text(), "H world");
        assert!(e.selection().is_empty());
    }

    #[test]
    fn backspace_and_delete_at_edges_are_noops() {
        let mut e = EditState::default();
        e.apply(EditOp::Backspace);
        e.apply(EditOp::Delete);
        assert_eq!(e.text(), "");
        let mut e = typed("ab");
        e.apply(EditOp::End(false));
        e.apply(EditOp::Delete); // at end: noop
        assert_eq!(e.text(), "ab");
    }

    #[test]
    fn multibyte_navigation_stays_on_char_boundaries() {
        let mut e = typed("aé🎉b");
        e.apply(EditOp::End(false));
        e.apply(EditOp::Left(false)); // before b
        e.apply(EditOp::Left(false)); // before 🎉
        e.apply(EditOp::Backspace); // deletes é
        assert_eq!(e.text(), "a🎉b");
        e.apply(EditOp::Delete); // deletes 🎉
        assert_eq!(e.text(), "ab");
    }

    #[test]
    fn set_caret_clamps_to_char_boundary() {
        let mut e = typed("é"); // 2 bytes
        e.apply(EditOp::SetCaret { byte: 1, select: false }); // mid-char
        assert_eq!(e.caret(), 0);
        e.apply(EditOp::SetCaret { byte: 99, select: false });
        assert_eq!(e.caret(), 2);
    }

    #[test]
    fn word_navigation() {
        let mut e = typed("foo bar_baz  qux");
        e.apply(EditOp::Home(false));
        e.apply(EditOp::WordRight(false));
        assert_eq!(e.caret(), 3); // end of foo
        e.apply(EditOp::WordRight(false));
        assert_eq!(e.caret(), 11); // end of bar_baz
        e.apply(EditOp::End(false));
        e.apply(EditOp::WordLeft(false));
        assert_eq!(e.caret(), 13); // start of qux
    }

    #[test]
    fn left_collapses_selection_to_its_start() {
        let mut e = typed("abcd");
        e.apply(EditOp::Home(false));
        e.apply(EditOp::Right(true));
        e.apply(EditOp::Right(true)); // "ab" selected
        e.apply(EditOp::Left(false));
        assert!(e.selection().is_empty());
        assert_eq!(e.caret(), 0);
    }

    #[test]
    fn undo_groups_whole_words_not_chars() {
        let mut e = typed("hello world");
        e.apply(EditOp::Undo);
        assert_eq!(e.text(), "hello ", "undo removes the last word as one step");
        e.apply(EditOp::Undo);
        assert_eq!(e.text(), "hello", "then the space");
        e.apply(EditOp::Undo);
        assert_eq!(e.text(), "", "then the first word");
    }

    #[test]
    fn redo_round_trips_and_new_edits_clear_redo() {
        let mut e = typed("abc def");
        e.apply(EditOp::Undo);
        e.apply(EditOp::Redo);
        assert_eq!(e.text(), "abc def");
        e.apply(EditOp::Undo);
        e.apply(EditOp::Insert('!'));
        e.apply(EditOp::Redo); // redo stack was cleared
        assert_eq!(e.text(), "abc !");
    }

    #[test]
    fn backspace_run_is_one_undo_step() {
        let mut e = typed("abcdef");
        for _ in 0..3 {
            e.apply(EditOp::Backspace);
        }
        assert_eq!(e.text(), "abc");
        e.apply(EditOp::Undo);
        assert_eq!(e.text(), "abcdef", "delete run undoes as one step");
    }

    #[test]
    fn select_all_then_type_replaces_everything() {
        let mut e = typed("old text");
        e.apply(EditOp::SelectAll);
        e.apply(EditOp::Insert('n'));
        assert_eq!(e.text(), "n");
        e.apply(EditOp::Undo);
        assert_eq!(e.text(), "old text");
    }

    #[test]
    fn paste_is_a_single_undo_step() {
        let mut e = typed("x: ");
        e.apply(EditOp::InsertStr("pasted content".into()));
        assert_eq!(e.text(), "x: pasted content");
        e.apply(EditOp::Undo);
        assert_eq!(e.text(), "x: ");
    }
}
