//! Display-only CommonMark parse. Styled text is not an editor (§24).
//!
//! Block structure plus inline spans: emphasis, strong, strike, code,
//! markdown links, `[[wikilink]]` / `![[embed]]`, GFM tables, and
//! `$math$` / `$$blocks$$` as source spans (no KaTeX). The same crate
//! is the substrate for later Tab paint — do not start a second parser.

mod ast;
mod editor;
mod parse;
mod wikilink;

pub use ast::{flatten_blocks, flatten_inlines, Block, Document, Inline, Wikilink};
pub use editor::{
    flatten_runs, hard_line_ranges, inline_runs, line_is_active, line_view, BlockKind, DisplayRun,
    LineView, RunStyle,
};
pub use parse::{parse, parse_preview, split_frontmatter};
pub use wikilink::{split as split_wikilinks, Piece as WikilinkPiece};

pub fn flatten(blocks: &[Block]) -> String {
    flatten_blocks(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_frontmatter_and_duplicate_h1() {
        let blocks = parse_preview(
            "---\ntitle: Atlas\n---\n\n# Atlas\n\nThe atlas.\n\n## More\n\nbody",
            "Atlas",
        );
        assert_eq!(flatten(&blocks), "The atlas.\n\nMore\n\nbody");
        assert!(matches!(&blocks[0], Block::Paragraph { .. }));
        assert_eq!(blocks[0].plain(), "The atlas.");
        assert!(matches!(&blocks[1], Block::Heading { level: 2, .. }));
        assert_eq!(blocks[1].plain(), "More");
    }

    #[test]
    fn lists_and_fences() {
        let doc = parse("- one\n- two\n\n```rust\ncode\n```\n");
        assert!(matches!(
            &doc.blocks[0],
            Block::ListItem { ordered: false, inlines, .. } if flatten_inlines(inlines) == "one"
        ));
        assert!(matches!(
            &doc.blocks[1],
            Block::ListItem { ordered: false, inlines, .. } if flatten_inlines(inlines) == "two"
        ));
        assert!(matches!(
            &doc.blocks[2],
            Block::Code { lang: Some(lang), text } if lang == "rust" && text == "code"
        ));
    }

    #[test]
    fn ordered_list_numbers_from_one() {
        let doc = parse("1. first\n2. second\n");
        assert!(matches!(
            &doc.blocks[0],
            Block::ListItem { ordered: true, index: 1, .. }
        ));
        assert!(matches!(
            &doc.blocks[1],
            Block::ListItem { ordered: true, index: 2, .. }
        ));
    }

    #[test]
    fn quote_and_rule() {
        let doc = parse("> a thought\n\n---\n\nnext");
        assert_eq!(doc.blocks[0].plain(), "a thought");
        assert!(matches!(&doc.blocks[1], Block::Rule));
        assert_eq!(doc.blocks[2].plain(), "next");
    }

    #[test]
    fn inlines_strong_code_link_and_wikilink() {
        let doc = parse("see **bold** and `x` plus [here](https://ex) and [[Atlas|the atlas]]");
        let Block::Paragraph { inlines } = &doc.blocks[0] else {
            panic!("{:?}", doc.blocks[0]);
        };
        assert!(
            inlines.iter().any(|i| matches!(i, Inline::Strong(t) if t == "bold")),
            "{inlines:?}"
        );
        assert!(
            inlines.iter().any(|i| matches!(i, Inline::Code(t) if t == "x")),
            "{inlines:?}"
        );
        assert!(
            inlines.iter().any(|i| matches!(i, Inline::Link { text, dest } if text == "here" && dest == "https://ex")),
            "{inlines:?}"
        );
        assert!(
            inlines.iter().any(|i| matches!(
                i,
                Inline::Wikilink(Wikilink { target, alias: Some(a), .. })
                    if target == "Atlas" && a == "the atlas"
            )),
            "{inlines:?}"
        );
        assert_eq!(flatten(&doc.blocks), "see bold and x plus here and the atlas");
    }

    #[test]
    fn wikilink_alone() {
        let doc = parse("[[Atlas|the atlas]]");
        assert!(matches!(
            &doc.blocks[0],
            Block::Paragraph { inlines } if matches!(
                &inlines[0],
                Inline::Wikilink(Wikilink { target, alias: Some(a), .. })
                    if target == "Atlas" && a == "the atlas"
            )
        ));
    }

    #[test]
    fn task_list_keeps_checked() {
        let doc = parse("- [x] done\n- [ ] todo\n");
        assert!(matches!(
            &doc.blocks[0],
            Block::ListItem { checked: Some(true), .. }
        ));
        assert!(matches!(
            &doc.blocks[1],
            Block::ListItem { checked: Some(false), .. }
        ));
        assert_eq!(doc.blocks[0].plain(), "done");
    }

    #[test]
    fn gfm_table_keeps_cells() {
        let doc = parse("| a | b |\n| --- | --- |\n| 1 | 2 |\n");
        assert!(
            matches!(
                &doc.blocks[0],
                Block::Table { headers, rows }
                    if flatten_inlines(&headers[0]) == "a"
                        && flatten_inlines(&headers[1]) == "b"
                        && flatten_inlines(&rows[0][0]) == "1"
                        && flatten_inlines(&rows[0][1]) == "2"
            ),
            "{:?}",
            doc.blocks[0]
        );
        assert_eq!(flatten(&doc.blocks), "a | b\n1 | 2");
    }

    #[test]
    fn math_inline_and_display() {
        let doc = parse("see $x^2$ and\n\n$$E=mc^2$$\n");
        let Block::Paragraph { inlines } = &doc.blocks[0] else {
            panic!("{:?}", doc.blocks[0]);
        };
        assert!(
            inlines.iter().any(|i| matches!(i, Inline::Math(t) if t == "x^2")),
            "{inlines:?}"
        );
        assert!(
            matches!(&doc.blocks[1], Block::Math { text } if text == "E=mc^2"),
            "{:?}",
            doc.blocks[1]
        );
    }
}
