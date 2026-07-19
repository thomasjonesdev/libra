//! libra-text — the text-entry editing core: a pure state machine
//! (`EditState × EditOp → EditState`) with per-field undo/redo grouped at word
//! boundaries. No clocks, platform, or fonts — components, the harness, and
//! (later) IME all drive this same machine.
//!
//! Architecture: ARCHITECTURE.md §10.1 (text entry).

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
    /// Caret to the start/end of the *current line* (== text start/end
    /// for single-line values, so Input behavior is unchanged).
    Home(bool),
    End(bool),
    /// Multi-line caret navigation (§10.1, Textarea): previous/next
    /// line at the same column (char offset from line start), clamped
    /// to the target line's length. First line Up → text start; last
    /// line Down → text end. Sticky column is a later refinement —
    /// the column is recomputed from the caret each move.
    Up(bool),
    Down(bool),
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
                let to = line_start(&self.text, self.caret);
                self.move_caret(to, select);
            }
            EditOp::End(select) => {
                self.last_kind = OpKind::Other;
                let to = line_end(&self.text, self.caret);
                self.move_caret(to, select);
            }
            EditOp::Up(select) => {
                self.last_kind = OpKind::Other;
                let start = line_start(&self.text, self.caret);
                let to = if start == 0 {
                    0 // first line: platform convention is caret to start
                } else {
                    let col = self.text[start..self.caret].chars().count();
                    let prev_start = line_start(&self.text, start - 1);
                    advance_columns(&self.text, prev_start, start - 1, col)
                };
                self.move_caret(to, select);
            }
            EditOp::Down(select) => {
                self.last_kind = OpKind::Other;
                let end = line_end(&self.text, self.caret);
                let to = if end == self.text.len() {
                    self.text.len() // last line: caret to end
                } else {
                    let start = line_start(&self.text, self.caret);
                    let col = self.text[start..self.caret].chars().count();
                    let next_start = end + 1;
                    let next_end = line_end(&self.text, next_start);
                    advance_columns(&self.text, next_start, next_end, col)
                };
                self.move_caret(to, select);
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

/// Byte offset of the current line's first char (0 on the first line).
fn line_start(text: &str, caret: usize) -> usize {
    text[..caret].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

/// Byte offset of the current line's `\n` (== text end on the last line).
fn line_end(text: &str, caret: usize) -> usize {
    caret + text[caret..].find('\n').unwrap_or(text.len() - caret)
}

/// Advance from `start` by up to `cols` chars, stopping at `end` —
/// "same column on the target line, clamped to its length."
fn advance_columns(text: &str, start: usize, end: usize, cols: usize) -> usize {
    text[start..end].char_indices().nth(cols).map(|(i, _)| start + i).unwrap_or(end)
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

    // ---- multi-line (§10.1 Textarea core) ----

    #[test]
    fn up_and_down_keep_the_column_when_it_fits() {
        let mut e = typed("alpha\nbravo\ncharlie");
        e.apply(EditOp::SetCaret { byte: 15, select: false }); // "cha|rlie" col 3
        e.apply(EditOp::Up(false));
        assert_eq!(e.caret(), 9, "bravo col 3: 'bra|vo'");
        e.apply(EditOp::Up(false));
        assert_eq!(e.caret(), 3, "alpha col 3: 'alp|ha'");
        e.apply(EditOp::Down(false));
        assert_eq!(e.caret(), 9);
        e.apply(EditOp::Down(false));
        assert_eq!(e.caret(), 15);
    }

    #[test]
    fn up_and_down_clamp_to_shorter_lines() {
        let mut e = typed("long first line\nab\nlong third line");
        e.apply(EditOp::SetCaret { byte: 10, select: false }); // col 10, line 1
        e.apply(EditOp::Down(false));
        assert_eq!(e.caret(), 18, "line 2 has 2 chars: clamps to its end ('ab|')");
        e.apply(EditOp::Down(false));
        assert_eq!(e.caret(), 21, "column re-derived from caret (col 2), not sticky");
    }

    #[test]
    fn up_on_first_line_goes_to_start_down_on_last_goes_to_end() {
        let mut e = typed("one\ntwo");
        e.apply(EditOp::SetCaret { byte: 2, select: false });
        e.apply(EditOp::Up(false));
        assert_eq!(e.caret(), 0);
        e.apply(EditOp::SetCaret { byte: 5, select: false });
        e.apply(EditOp::Down(false));
        assert_eq!(e.caret(), e.text().len());
    }

    #[test]
    fn home_and_end_are_line_scoped_on_multiline_text() {
        let mut e = typed("first\nsecond");
        e.apply(EditOp::SetCaret { byte: 9, select: false }); // "sec|ond"
        e.apply(EditOp::Home(false));
        assert_eq!(e.caret(), 6, "start of the *line*, not the text");
        e.apply(EditOp::End(false));
        assert_eq!(e.caret(), 12, "end of the line == end of text on the last line");
        // Single-line values keep the old text-start/end behavior.
        let mut s = typed("plain");
        s.apply(EditOp::Home(false));
        assert_eq!(s.caret(), 0);
        s.apply(EditOp::End(false));
        assert_eq!(s.caret(), 5);
    }

    #[test]
    fn shift_up_extends_a_selection_across_lines() {
        let mut e = typed("aa\nbb\ncc");
        e.apply(EditOp::SetCaret { byte: 7, select: false }); // "c|c"
        e.apply(EditOp::Up(true));
        assert_eq!(e.selection(), 4..7, "'b' to 'c' selected, caret on line 2 col 1");
    }

    #[test]
    fn up_down_are_multibyte_safe() {
        let mut e = typed("héllo\nwörld");
        e.apply(EditOp::End(false));
        e.apply(EditOp::Up(false));
        assert_eq!(e.caret(), e.text().find('\n').unwrap(), "clamps to a char boundary");
        e.apply(EditOp::Down(false));
        assert_eq!(e.caret(), e.text().len());
    }
}
