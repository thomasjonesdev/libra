//! Block + inline AST. Display-only — this is not an editor model (§24).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Heading { level: u8, inlines: Vec<Inline> },
    Paragraph { inlines: Vec<Inline> },
    ListItem {
        ordered: bool,
        index: u32,
        checked: Option<bool>,
        inlines: Vec<Inline>,
    },
    Code { lang: Option<String>, text: String },
    Quote { inlines: Vec<Inline> },
    Table {
        headers: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    Math { text: String },
    Rule,
}

impl Block {
    pub fn plain(&self) -> String {
        match self {
            Block::Heading { inlines, .. }
            | Block::Paragraph { inlines }
            | Block::ListItem { inlines, .. }
            | Block::Quote { inlines } => flatten_inlines(inlines),
            Block::Code { text, .. } | Block::Math { text } => text.clone(),
            Block::Table { headers, rows } => {
                let mut lines = Vec::new();
                if !headers.is_empty() {
                    lines.push(
                        headers
                            .iter()
                            .map(|c| flatten_inlines(c))
                            .collect::<Vec<_>>()
                            .join(" | "),
                    );
                }
                for row in rows {
                    lines.push(
                        row.iter()
                            .map(|c| flatten_inlines(c))
                            .collect::<Vec<_>>()
                            .join(" | "),
                    );
                }
                lines.join("\n")
            }
            Block::Rule => String::new(),
        }
    }

    pub fn inlines(&self) -> &[Inline] {
        match self {
            Block::Heading { inlines, .. }
            | Block::Paragraph { inlines }
            | Block::ListItem { inlines, .. }
            | Block::Quote { inlines } => inlines,
            Block::Code { .. } | Block::Table { .. } | Block::Math { .. } | Block::Rule => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Strong(String),
    Emphasis(String),
    Strike(String),
    Code(String),
    Link { text: String, dest: String },
    Wikilink(Wikilink),
    Math(String),
}

impl Inline {
    pub fn plain(&self) -> String {
        match self {
            Inline::Text(s)
            | Inline::Strong(s)
            | Inline::Emphasis(s)
            | Inline::Strike(s)
            | Inline::Code(s) => s.clone(),
            Inline::Link { text, .. } => text.clone(),
            Inline::Wikilink(w) => w.display().to_string(),
            Inline::Math(s) => s.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wikilink {
    pub target: String,
    pub alias: Option<String>,
    pub heading: Option<String>,
    pub embed: bool,
}

impl Wikilink {
    pub fn display(&self) -> &str {
        self.alias.as_deref().unwrap_or(self.target.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Document {
    pub frontmatter: Option<String>,
    pub blocks: Vec<Block>,
}

pub fn flatten_inlines(inlines: &[Inline]) -> String {
    inlines.iter().map(Inline::plain).collect::<Vec<_>>().join("")
}

pub fn flatten_blocks(blocks: &[Block]) -> String {
    blocks
        .iter()
        .map(Block::plain)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}
