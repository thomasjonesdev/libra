//! Title-mention missing links. The first "340 connections" slice:
//! notes that name another note but never wikilinked it.
//!
//! Embeddings and hybrid rank come later. This is the cheap, local
//! signal that already exists in the files.

use gloss_core::{comment_spans, split_frontmatter, wikilinks, ByteRange, NoteId, Workspace};

use crate::{stem_title, Index, IndexError, IndexedNote};

const MIN_ALIAS_LEN: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestedLink {
    pub src: NoteId,
    pub dst: NoteId,
    pub matched: String,
    pub byte_start: usize,
}

impl Index {
    /// Mentions of other notes' titles or file stems that are not already
    /// wikilinked. One hit per `(src, dst)`, first match in the body.
    pub fn suggest_links(&self, ws: &Workspace) -> Result<Vec<SuggestedLink>, IndexError> {
        let notes = self.notes()?;
        let aliases = aliases_for(&notes);
        let mut out = Vec::new();

        for src in &notes {
            let mut already = self
                .outgoing(&src.id)?
                .into_iter()
                .filter_map(|l| l.dst)
                .collect::<Vec<_>>();
            let bytes = ws.read(&src.id.vault_path())?;
            let file = String::from_utf8_lossy(&bytes);
            let (body, body_off) = body_view(&file);
            let skips = skip_ranges(&file, body_off, body.len());

            for (dst, alias) in &aliases {
                if dst == &src.id || already.contains(dst) {
                    continue;
                }
                if let Some(at) = find_alias(body, alias, &skips) {
                    already.push(dst.clone());
                    out.push(SuggestedLink {
                        src: src.id.clone(),
                        dst: dst.clone(),
                        matched: alias.clone(),
                        byte_start: body_off + at,
                    });
                }
            }
        }

        out.sort_by(|a, b| {
            a.src
                .path
                .as_str()
                .cmp(b.src.path.as_str())
                .then(a.dst.path.as_str().cmp(b.dst.path.as_str()))
        });
        Ok(out)
    }
}

fn aliases_for(notes: &[IndexedNote]) -> Vec<(NoteId, String)> {
    let mut out = Vec::new();
    for note in notes {
        let stem = stem_title(note.id.path.as_str());
        let mut seen = Vec::new();
        for alias in [note.title.as_str(), stem.as_str()] {
            if alias.chars().count() < MIN_ALIAS_LEN {
                continue;
            }
            if seen.iter().any(|s: &String| s.eq_ignore_ascii_case(alias)) {
                continue;
            }
            seen.push(alias.to_string());
            out.push((note.id.clone(), alias.to_string()));
        }
    }
    out.sort_by(|a, b| b.1.chars().count().cmp(&a.1.chars().count()));
    out
}

fn body_view(src: &str) -> (&str, usize) {
    match split_frontmatter(src) {
        Some(split) => (split.body, split.body_range.start),
        None => (src, 0),
    }
}

fn skip_ranges(src: &str, body_off: usize, body_len: usize) -> Vec<ByteRange> {
    let mut skips = Vec::new();
    for range in comment_spans(src)
        .into_iter()
        .chain(wikilinks(src).into_iter().map(|l| l.range))
    {
        if let Some(shifted) = shift_into_body(range, body_off, body_len) {
            skips.push(shifted);
        }
    }
    skips
}

fn shift_into_body(range: ByteRange, off: usize, body_len: usize) -> Option<ByteRange> {
    let start = range.start.max(off).saturating_sub(off);
    let end = range.end.max(off).saturating_sub(off).min(body_len);
    ByteRange::new(start, end).filter(|r| r.start < r.end)
}

fn find_alias(body: &str, alias: &str, skips: &[ByteRange]) -> Option<usize> {
    let needle: Vec<char> = alias.chars().collect();
    if needle.is_empty() {
        return None;
    }
    let chars: Vec<(usize, char)> = body.char_indices().collect();
    let n = needle.len();
    if chars.len() < n {
        return None;
    }
    for i in 0..=chars.len() - n {
        let start = chars[i].0;
        let end = if i + n < chars.len() {
            chars[i + n].0
        } else {
            body.len()
        };
        let same = chars[i..i + n]
            .iter()
            .zip(needle.iter())
            .all(|((_, a), b)| chars_match(*a, *b));
        if same && is_boundary(body, start, end) && !overlaps(start, end, skips) {
            return Some(start);
        }
    }
    None
}

fn chars_match(a: char, b: char) -> bool {
    a.eq_ignore_ascii_case(&b) || a.to_lowercase().eq(b.to_lowercase())
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_boundary(src: &str, start: usize, end: usize) -> bool {
    let before_ok = src
        .get(..start)
        .and_then(|s| s.chars().next_back())
        .map(|c| !is_word_char(c))
        .unwrap_or(true);
    let after_ok = src
        .get(end..)
        .and_then(|s| s.chars().next())
        .map(|c| !is_word_char(c))
        .unwrap_or(true);
    before_ok && after_ok
}

fn overlaps(start: usize, end: usize, skips: &[ByteRange]) -> bool {
    skips.iter().any(|r| start < r.end && end > r.start)
}

#[cfg(test)]
mod matcher_tests {
    use super::{find_alias, is_boundary, skip_ranges};
    use gloss_core::ByteRange;

    #[test]
    fn whole_word_only() {
        assert!(find_alias("the Atlas.", "Atlas", &[]).is_some());
        assert!(find_alias("Atlases gather", "Atlas", &[]).is_none());
        assert!(is_boundary("Atlas", 0, 5));
    }

    #[test]
    fn skips_wikilink_and_comment() {
        let src = "see [[Atlas]] and %% Atlas %% then Atlas.";
        let skips = skip_ranges(src, 0, src.len());
        assert_eq!(
            find_alias(src, "Atlas", &skips),
            Some(src.rfind("Atlas").unwrap())
        );
        assert!(find_alias("%% Atlas %%", "Atlas", &skip_ranges("%% Atlas %%", 0, 11)).is_none());
    }

    #[test]
    fn skip_overlap() {
        let skips = [ByteRange { start: 4, end: 9 }];
        assert!(find_alias("the Atlas", "Atlas", &skips).is_none());
    }
}
