//! libra-text — the text-entry editing core: a pure state machine
//! (`EditState × EditOp → EditState`) with per-field undo/redo grouped at word
//! boundaries. No clocks, platform, or fonts — components, the harness, and
//! IME all drive this same machine. A buffer at or above [`ROPE_THRESHOLD`]
//! keeps a `ropey` rope so undo clones stay cheap (§10.1).
//!
//! Architecture: ARCHITECTURE.md §10.1 (text entry).

/// Bytes at which the live buffer also keeps a rope. Below this, the
/// buffer is a `String` and undo stores strings, which is the right cost
/// for ordinary fields.
pub const ROPE_THRESHOLD: usize = 64 * 1024;

/// Uncommitted IME composition. It is not part of [`EditState::text`]
/// until [`EditOp::Commit`]. `at` is a byte offset in the committed text;
/// `replaces` is how many committed bytes (a selection) the composition
/// covers. `cursor` is a byte range inside `text` (`None` hides the caret).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preedit {
    pub text: String,
    pub at: usize,
    pub replaces: usize,
    pub cursor: Option<(usize, usize)>,
}

impl Preedit {
    /// Committed text with this composition spliced in at `at`.
    pub fn splice(&self, committed: &str) -> String {
        let at = self.at.min(committed.len());
        let end = (self.at + self.replaces).min(committed.len());
        let mut out = String::with_capacity(committed.len() - (end - at) + self.text.len());
        out.push_str(&committed[..at]);
        out.push_str(&self.text);
        out.push_str(&committed[end..]);
        out
    }
}

/// Caret + selection over a single-line string. `caret` and `anchor`
/// are byte indices on char boundaries; `caret == anchor` means no
/// selection. All mutation goes through [`EditState::apply`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EditState {
    text: String,
    caret: usize,
    anchor: usize,
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    /// Last op class, for undo grouping (word-boundary snapshots).
    last_kind: OpKind,
    /// Char column remembered across Up/Down so a short line does not
    /// forget the column the caret came from (§10.1 sticky column).
    preferred_col: Option<usize>,
    preedit: Option<Preedit>,
    /// Present once `text` reaches [`ROPE_THRESHOLD`]. Undo clones this
    /// instead of the string.
    rope: Option<ropey::Rope>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Snap {
    Small(String, usize, usize),
    Rope(ropey::Rope, usize, usize),
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
    /// line at the remembered column (char offset from line start),
    /// clamped to the target line's length. First line Up → text start;
    /// last line Down → text end. The column sticks until a non-vertical
    /// edit. Visual-line movement is the shell's job: this core has no
    /// font metrics.
    Up(bool),
    Down(bool),
    SelectAll,
    /// Select the word (or whitespace/punctuation run) at the caret.
    SelectWord,
    /// Select the current hard line (excluding the trailing newline).
    SelectLine,
    /// Place the caret at a byte offset (clamped to a char boundary).
    SetCaret { byte: usize, select: bool },
    Undo,
    Redo,
    /// Show or replace the uncommitted composition. An empty `text`
    /// clears it and leaves the buffer alone.
    Preedit { text: String, cursor: Option<(usize, usize)> },
    /// Insert `text` in place of the composition (or the selection) as
    /// one undo step.
    Commit(String),
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

    pub fn preedit(&self) -> Option<&Preedit> {
        self.preedit.as_ref()
    }

    /// Caret in the spliced display string. Equals [`Self::caret`] when
    /// nothing is composing.
    pub fn display_caret(&self) -> usize {
        match &self.preedit {
            Some(p) => {
                let within = p
                    .cursor
                    .map(|(_, end)| clamp_boundary(&p.text, end))
                    .unwrap_or(p.text.len());
                p.at + within
            }
            None => self.caret,
        }
    }

    /// An IME may ask for the caret to stay hidden during composition.
    pub fn show_caret(&self) -> bool {
        self.preedit.as_ref().map(|p| p.cursor.is_some()).unwrap_or(true)
    }

    /// The state machine. Every editing affordance — keyboard, harness
    /// verbs, IME commit — reduces to these transitions.
    pub fn apply(&mut self, op: EditOp) {
        if !matches!(op, EditOp::Up(_) | EditOp::Down(_)) {
            self.preferred_col = None;
        }
        if !matches!(op, EditOp::Preedit { .. } | EditOp::Commit(_)) {
            self.commit_preedit_for_edit();
        }
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
                let col = self.column_from(start);
                let to = if start == 0 {
                    0 // first line: platform convention is caret to start
                } else {
                    let prev_start = line_start(&self.text, start - 1);
                    advance_columns(&self.text, prev_start, start - 1, col)
                };
                self.preferred_col = Some(col);
                self.move_caret(to, select);
            }
            EditOp::Down(select) => {
                self.last_kind = OpKind::Other;
                let end = line_end(&self.text, self.caret);
                let start = line_start(&self.text, self.caret);
                let col = self.column_from(start);
                let to = if end == self.text.len() {
                    self.text.len() // last line: caret to end
                } else {
                    let next_start = end + 1;
                    let next_end = line_end(&self.text, next_start);
                    advance_columns(&self.text, next_start, next_end, col)
                };
                self.preferred_col = Some(col);
                self.move_caret(to, select);
            }
            EditOp::SelectAll => {
                self.last_kind = OpKind::Other;
                self.anchor = 0;
                self.caret = self.text.len();
            }
            EditOp::SelectWord => {
                self.last_kind = OpKind::Other;
                let (start, end) = word_bounds(&self.text, self.caret);
                self.anchor = start;
                self.caret = end;
            }
            EditOp::SelectLine => {
                self.last_kind = OpKind::Other;
                self.anchor = line_start(&self.text, self.caret);
                self.caret = line_end(&self.text, self.caret);
            }
            EditOp::SetCaret { byte, select } => {
                self.last_kind = OpKind::Other;
                let to = clamp_boundary(&self.text, byte);
                self.move_caret(to, select);
            }
            EditOp::Undo => {
                self.last_kind = OpKind::Other;
                self.preedit = None;
                if let Some(snap) = self.undo.pop() {
                    self.redo.push(self.capture());
                    self.restore(snap);
                }
            }
            EditOp::Redo => {
                self.last_kind = OpKind::Other;
                self.preedit = None;
                if let Some(snap) = self.redo.pop() {
                    self.undo.push(self.capture());
                    self.restore(snap);
                }
            }
            EditOp::Preedit { text, cursor } => {
                self.last_kind = OpKind::Other;
                if text.is_empty() {
                    self.preedit = None;
                    return;
                }
                let cursor = cursor.map(|(a, b)| {
                    (clamp_boundary(&text, a), clamp_boundary(&text, b))
                });
                if let Some(existing) = &mut self.preedit {
                    existing.text = text;
                    existing.cursor = cursor;
                } else {
                    let at = self.selection().start;
                    let replaces = self.selection().end - at;
                    self.preedit = Some(Preedit { text, at, replaces, cursor });
                }
            }
            EditOp::Commit(text) => {
                if let Some(p) = self.preedit.take() {
                    self.caret = p.at;
                    self.anchor = p.at + p.replaces;
                }
                self.last_kind = OpKind::Other;
                self.snapshot_if(true);
                self.replace_selection(&text);
            }
        }
    }

    /// A real edit during composition commits the preedit first, as its
    /// own undo step, then the edit proceeds.
    fn commit_preedit_for_edit(&mut self) {
        let Some(p) = self.preedit.take() else {
            return;
        };
        if p.text.is_empty() {
            return;
        }
        self.caret = p.at;
        self.anchor = p.at + p.replaces;
        self.snapshot_if(true);
        self.last_kind = OpKind::Other;
        self.replace_selection(&p.text);
    }

    fn column_from(&self, line_start: usize) -> usize {
        self.preferred_col
            .unwrap_or_else(|| self.text[line_start..self.caret].chars().count())
    }

    fn capture(&self) -> Snap {
        if let Some(rope) = &self.rope {
            Snap::Rope(rope.clone(), self.caret, self.anchor)
        } else {
            Snap::Small(self.text.clone(), self.caret, self.anchor)
        }
    }

    fn restore(&mut self, snap: Snap) {
        match snap {
            Snap::Small(text, caret, anchor) => {
                self.text = text;
                self.rope = None;
                self.caret = caret;
                self.anchor = anchor;
            }
            Snap::Rope(rope, caret, anchor) => {
                self.text = rope.to_string();
                self.caret = caret.min(self.text.len());
                self.anchor = anchor.min(self.text.len());
                self.rope = Some(rope);
            }
        }
    }

    fn snapshot_if(&mut self, cond: bool) {
        if cond {
            self.undo.push(self.capture());
            self.redo.clear();
        }
    }

    fn replace_selection(&mut self, insert: &str) {
        let sel = self.selection();
        if let Some(rope) = &mut self.rope {
            let start = rope.byte_to_char(sel.start);
            let end = rope.byte_to_char(sel.end);
            rope.remove(start..end);
            if !insert.is_empty() {
                rope.insert(start, insert);
            }
        }
        self.text.replace_range(sel.clone(), insert);
        self.caret = sel.start + insert.len();
        self.anchor = self.caret;
        if self.text.len() >= ROPE_THRESHOLD {
            if self.rope.is_none() {
                self.rope = Some(ropey::Rope::from_str(&self.text));
            }
        } else {
            self.rope = None;
        }
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

/// Inclusive run of the same class (word vs non-word) around `caret`.
fn word_bounds(text: &str, caret: usize) -> (usize, usize) {
    if text.is_empty() {
        return (0, 0);
    }
    let i = if caret >= text.len() {
        prev_boundary(text, text.len())
    } else {
        caret
    };
    let Some(ch) = text[i..].chars().next() else {
        return (0, 0);
    };
    let wordish = is_word(ch);
    let mut start = i;
    while start > 0 {
        let p = prev_boundary(text, start);
        let c = text[p..].chars().next().unwrap();
        if is_word(c) != wordish {
            break;
        }
        start = p;
    }
    let mut end = i + ch.len_utf8();
    while end < text.len() {
        let c = text[end..].chars().next().unwrap();
        if is_word(c) != wordish {
            break;
        }
        end += c.len_utf8();
    }
    (start, end)
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
        assert_eq!(e.caret(), 29, "sticky column 10 lands on the long third line");
        e.apply(EditOp::Up(false));
        assert_eq!(e.caret(), 18, "the short line clamps again");
        e.apply(EditOp::Up(false));
        assert_eq!(e.caret(), 10, "the remembered column is restored");
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

    #[test]
    fn select_word_covers_the_run_under_the_caret() {
        let mut e = typed("hello world");
        e.apply(EditOp::SetCaret { byte: 1, select: false });
        e.apply(EditOp::SelectWord);
        assert_eq!(e.selected_text(), "hello");
        e.apply(EditOp::SetCaret { byte: 8, select: false });
        e.apply(EditOp::SelectWord);
        assert_eq!(e.selected_text(), "world");
    }

    #[test]
    fn preedit_stays_out_of_the_buffer_until_commit() {
        let mut e = typed("go");
        e.apply(EditOp::Preedit { text: "あ".into(), cursor: Some((3, 3)) });
        assert_eq!(e.text(), "go");
        assert_eq!(e.preedit().map(|p| p.text.as_str()), Some("あ"));
        assert_eq!(e.display_caret(), 2 + 3);
        e.apply(EditOp::Commit("あ".into()));
        assert!(e.preedit().is_none());
        assert_eq!(e.text(), "goあ");
        e.apply(EditOp::Undo);
        assert_eq!(e.text(), "go");
    }

    #[test]
    fn an_edit_during_composition_commits_first() {
        let mut e = typed("");
        e.apply(EditOp::Preedit { text: "ab".into(), cursor: Some((2, 2)) });
        e.apply(EditOp::Insert('!'));
        assert_eq!(e.text(), "ab!");
        e.apply(EditOp::Undo);
        assert_eq!(e.text(), "ab");
        e.apply(EditOp::Undo);
        assert_eq!(e.text(), "");
    }

    #[test]
    fn empty_preedit_clears_without_touching_text() {
        let mut e = typed("hi");
        e.apply(EditOp::Preedit { text: "x".into(), cursor: None });
        assert!(!e.show_caret());
        e.apply(EditOp::Preedit { text: String::new(), cursor: None });
        assert!(e.preedit().is_none());
        assert_eq!(e.text(), "hi");
        assert!(e.show_caret());
    }

    #[test]
    fn commit_replaces_a_selection() {
        let mut e = typed("hello");
        e.apply(EditOp::SelectAll);
        e.apply(EditOp::Preedit { text: "よ".into(), cursor: Some((3, 3)) });
        let shown = e.preedit().unwrap().splice(e.text());
        assert_eq!(shown, "よ");
        e.apply(EditOp::Commit("よ".into()));
        assert_eq!(e.text(), "よ");
    }

    #[test]
    fn a_large_paste_undos_without_keeping_the_rope_below_the_threshold() {
        let mut e = EditState::default();
        let big = "x".repeat(ROPE_THRESHOLD);
        e.apply(EditOp::InsertStr(big.clone()));
        assert_eq!(e.text().len(), ROPE_THRESHOLD);
        e.apply(EditOp::Insert('!'));
        assert_eq!(e.text().len(), ROPE_THRESHOLD + 1);
        e.apply(EditOp::Undo);
        assert_eq!(e.text(), big);
        e.apply(EditOp::Undo);
        assert_eq!(e.text(), "");
    }

    #[test]
    fn select_line_excludes_the_newline() {
        let mut e = typed("one\ntwo\nthree");
        e.apply(EditOp::SetCaret { byte: 5, select: false });
        e.apply(EditOp::SelectLine);
        assert_eq!(e.selected_text(), "two");
    }
}
