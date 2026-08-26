/// Byte offsets into the original file. Inclusive start, exclusive end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

impl ByteRange {
    pub fn new(start: usize, end: usize) -> Option<Self> {
        if start <= end {
            Some(ByteRange { start, end })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    Wikilink,
    Embed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wikilink {
    pub raw_target: String,
    pub alias: Option<String>,
    pub heading: Option<String>,
    pub block_id: Option<String>,
    pub kind: LinkKind,
    pub range: ByteRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontmatterSplit<'a> {
    pub yaml: &'a str,
    pub body: &'a str,
    pub yaml_range: ByteRange,
    pub body_range: ByteRange,
}

/// Split YAML frontmatter. Unknown keys stay in `yaml` as raw text.
pub fn split_frontmatter(src: &str) -> Option<FrontmatterSplit<'_>> {
    let rest = src.strip_prefix("---")?;
    let rest = rest
        .strip_prefix('\n')
        .or_else(|| rest.strip_prefix("\r\n"))?;
    let yaml_start = src.len() - rest.len();
    let close = rest.find("\n---").or_else(|| rest.find("\r\n---"))?;
    let after_close_line = if rest[close..].starts_with("\r\n---") {
        close + "\r\n---".len()
    } else {
        close + "\n---".len()
    };
    let yaml = &rest[..close];
    let after = &rest[after_close_line..];
    let body = after
        .strip_prefix('\n')
        .or_else(|| after.strip_prefix("\r\n"))
        .unwrap_or(after);
    let body_start = src.len() - body.len();
    Some(FrontmatterSplit {
        yaml,
        body,
        yaml_range: ByteRange {
            start: yaml_start,
            end: yaml_start + yaml.len(),
        },
        body_range: ByteRange {
            start: body_start,
            end: src.len(),
        },
    })
}

/// Wikilinks and embeds. Skips `%%comment%%` spans.
pub fn wikilinks(src: &str) -> Vec<Wikilink> {
    let comments = comment_spans(src);
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if in_comment(i, &comments) {
            i += 1;
            continue;
        }
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
        if let Some(end_rel) = find_close(&src[inner_at..]) {
            let inner = &src[inner_at..inner_at + end_rel];
            let end = inner_at + end_rel + 2;
            if !in_comment(start, &comments) {
                if let Some(parsed) = parse_inner(inner, start, end, embed) {
                    out.push(parsed);
                }
            }
            i = end;
        } else {
            i += 1;
        }
    }
    out
}

fn find_close(s: &str) -> Option<usize> {
    s.find("]]")
}

fn parse_inner(inner: &str, start: usize, end: usize, embed: bool) -> Option<Wikilink> {
    let (target_part, alias) = match inner.split_once('|') {
        Some((t, a)) => (t, Some(a.to_string())),
        None => (inner, None),
    };
    let (raw_target, heading, block_id) = match target_part.split_once('#') {
        Some((t, frag)) => {
            if let Some(block) = frag.strip_prefix('^') {
                (t.to_string(), None, Some(block.to_string()))
            } else {
                (t.to_string(), Some(frag.to_string()), None)
            }
        }
        None => (target_part.to_string(), None, None),
    };
    if raw_target.is_empty() {
        return None;
    }
    Some(Wikilink {
        raw_target,
        alias,
        heading,
        block_id,
        kind: if embed {
            LinkKind::Embed
        } else {
            LinkKind::Wikilink
        },
        range: ByteRange { start, end },
    })
}

/// `%%comment%%` spans. Used by the wikilink scanner and missing-link suggestions.
pub fn comment_spans(src: &str) -> Vec<ByteRange> {
    let mut spans = Vec::new();
    let mut rest = src;
    let mut offset = 0;
    while let Some(start_rel) = rest.find("%%") {
        let start = offset + start_rel;
        let after = start + 2;
        if let Some(end_rel) = src[after..].find("%%") {
            let end = after + end_rel + 2;
            spans.push(ByteRange { start, end });
            offset = end;
            rest = &src[end..];
        } else {
            break;
        }
    }
    spans
}

fn in_comment(i: usize, comments: &[ByteRange]) -> bool {
    comments.iter().any(|c| i >= c.start && i < c.end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wikilink_forms() {
        let src = "see [[note]] and [[note|alias]] and [[note#Heading]] and [[note#^blockid]]";
        let links = wikilinks(src);
        assert_eq!(links.len(), 4);
        assert_eq!(links[0].raw_target, "note");
        assert_eq!(links[1].alias.as_deref(), Some("alias"));
        assert_eq!(links[2].heading.as_deref(), Some("Heading"));
        assert_eq!(links[3].block_id.as_deref(), Some("blockid"));
    }

    #[test]
    fn parses_embeds() {
        let src = "![[img.png|300]] and ![[note#Heading]]";
        let links = wikilinks(src);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].kind, LinkKind::Embed);
        assert_eq!(links[0].alias.as_deref(), Some("300"));
        assert_eq!(links[1].heading.as_deref(), Some("Heading"));
    }

    #[test]
    fn skips_commented_wikilinks() {
        let src = "keep [[live]] %% hide [[ghost]] %% still [[here]]";
        let links = wikilinks(src);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].raw_target, "live");
        assert_eq!(links[1].raw_target, "here");
    }

    #[test]
    fn frontmatter_round_trip_preserves_unknown_keys() {
        let src = "---\ntitle: Hi\nweird: 1\n---\n\nbody [[Atlas]]\n";
        let split = split_frontmatter(src).unwrap();
        assert!(split.yaml.contains("weird: 1"));
        assert_eq!(split.body, "\nbody [[Atlas]]\n");
        let rebuilt = format!("---\n{}\n---\n{}", split.yaml, split.body);
        assert_eq!(rebuilt, src);
    }

    #[test]
    fn wikilink_byte_range_matches_source() {
        let src = "xx[[n]]yy";
        let links = wikilinks(src);
        assert_eq!(&src[links[0].range.start..links[0].range.end], "[[n]]");
    }
}
