//! Display-time `[[wikilink]]` / `![[embed]]` split. The vault core keeps
//! the byte-accurate scanner (`gloss-core`); this one only carves spans
//! out of a text run so preview can paint them.

use crate::ast::Wikilink;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Text(String),
    Link(Wikilink),
}

pub fn split(src: &str) -> Vec<Piece> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut text_at = 0;
    while i < bytes.len() {
        let embed =
            bytes[i] == b'!' && bytes.get(i + 1) == Some(&b'[') && bytes.get(i + 2) == Some(&b'[');
        let start = if embed {
            i
        } else if bytes[i] == b'[' && bytes.get(i + 1) == Some(&b'[') {
            i
        } else {
            i += 1;
            continue;
        };
        let inner_at = if embed { i + 3 } else { i + 2 };
        if let Some(end_rel) = src[inner_at..].find("]]") {
            if start > text_at {
                out.push(Piece::Text(src[text_at..start].to_string()));
            }
            let inner = &src[inner_at..inner_at + end_rel];
            if let Some(link) = parse_inner(inner, embed) {
                out.push(Piece::Link(link));
            } else {
                out.push(Piece::Text(src[start..inner_at + end_rel + 2].to_string()));
            }
            i = inner_at + end_rel + 2;
            text_at = i;
        } else {
            i += 1;
        }
    }
    if text_at < src.len() {
        out.push(Piece::Text(src[text_at..].to_string()));
    }
    if out.is_empty() && !src.is_empty() {
        out.push(Piece::Text(src.to_string()));
    }
    out
}

fn parse_inner(inner: &str, embed: bool) -> Option<Wikilink> {
    let (target_part, alias) = match inner.split_once('|') {
        Some((t, a)) => (t, Some(a.to_string())),
        None => (inner, None),
    };
    let (target, heading) = match target_part.split_once('#') {
        Some((t, frag)) => (t.to_string(), Some(frag.trim_start_matches('^').to_string())),
        None => (target_part.to_string(), None),
    };
    if target.is_empty() {
        return None;
    }
    Some(Wikilink {
        target,
        alias,
        heading,
        embed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_alias_and_heading() {
        let parts = split("see [[Atlas|the atlas]] and [[Atlas#Atlas]]");
        assert!(matches!(&parts[0], Piece::Text(t) if t == "see "));
        assert!(matches!(
            &parts[1],
            Piece::Link(Wikilink { target, alias: Some(a), .. })
                if target == "Atlas" && a == "the atlas"
        ));
        assert!(matches!(&parts[3], Piece::Link(Wikilink { heading: Some(h), .. }) if h == "Atlas"));
    }

    #[test]
    fn embed_flag() {
        let parts = split("![[img.png]]");
        assert!(matches!(&parts[0], Piece::Link(Wikilink { embed: true, target, .. }) if target == "img.png"));
    }
}
