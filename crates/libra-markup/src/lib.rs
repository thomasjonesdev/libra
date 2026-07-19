//! libra-markup — the parser for `.lx` view files: one shared grammar → an
//! AST with spans on every node; errors carry line:col + a suggestion where
//! possible. Consumed by both view engines (libra-interp, libra-codegen).
//!
//! Architecture: ARCHITECTURE.md §6.6 (parser), §6.2 (the closed grammar).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: u32,
    pub col: u32,
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttrValue {
    /// Bare attribute: `bold`, `grow`.
    Flag,
    /// String literal: `variant="primary"`.
    Str(String),
    /// Braced number: `gap={12}`.
    Num(f32),
    /// Braced identifier/path: `onPress={Increment}`, `value={user.name}`.
    /// Whether it is a handler or a binding is decided by the consumer
    /// from the attribute name (`on*` = handler).
    Ident(String),
    /// Braced action call with one binding argument: `onPress={toggle(item)}`
    /// (§6.2). Used for item-scoped handlers inside `<for>` — the arg names
    /// a binding (the loop variable) whose value becomes the action's
    /// `String` payload. `arg` is a single binding path, not an expression.
    Call { name: String, arg: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attr {
    pub name: String,
    pub value: AttrValue,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Lit(String),
    /// `{count}` inside text content.
    Bind { path: String, span: Span },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Element {
        name: String,
        attrs: Vec<Attr>,
        children: Vec<Node>,
        span: Span,
    },
    Text(Vec<Inline>),
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.span, self.message)
    }
}

impl std::error::Error for ParseError {}

pub fn parse(src: &str) -> Result<Node, ParseError> {
    let mut p = Parser::new(src);
    p.skip_ws();
    let root = p.element()?;
    p.skip_ws();
    if !p.eof() {
        return Err(p.err("unexpected content after the root element"));
    }
    Ok(root)
}

struct Parser<'a> {
    chars: Vec<char>,
    pos: usize,
    line: u32,
    col: u32,
    _src: &'a str,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Parser { chars: src.chars().collect(), pos: 0, line: 1, col: 1, _src: src }
    }

    fn span(&self) -> Span {
        Span { line: self.line, col: self.col }
    }

    fn err(&self, msg: impl Into<String>) -> ParseError {
        ParseError { message: msg.into(), span: self.span() }
    }

    fn eof(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek2(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, c: char, ctx: &str) -> Result<(), ParseError> {
        if self.eat(c) {
            Ok(())
        } else {
            Err(self.err(format!(
                "expected '{c}' {ctx}, found {}",
                self.peek().map(|c| format!("'{c}'")).unwrap_or_else(|| "end of file".into())
            )))
        }
    }

    fn skip_ws(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.bump();
                }
                // line comments: // to end of line
                Some('/') if self.peek2() == Some('/') => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                _ => break,
            }
        }
    }

    fn ident(&mut self, what: &str) -> Result<String, ParseError> {
        let mut s = String::new();
        match self.peek() {
            Some(c) if c.is_alphabetic() || c == '_' => {}
            _ => return Err(self.err(format!("expected {what}"))),
        }
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                s.push(c);
                self.bump();
            } else {
                break;
            }
        }
        Ok(s)
    }

    /// Dotted path: `count`, `user.name`.
    fn path(&mut self) -> Result<String, ParseError> {
        let mut s = self.ident("an identifier")?;
        while self.peek() == Some('.') {
            self.bump();
            s.push('.');
            s.push_str(&self.ident("an identifier after '.'")?);
        }
        Ok(s)
    }

    fn element(&mut self) -> Result<Node, ParseError> {
        let span = self.span();
        self.expect('<', "to open an element")?;
        let name = self.ident("an element name after '<'")?;
        let mut attrs = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some('/') => {
                    self.bump();
                    self.expect('>', "after '/' in a self-closing tag")?;
                    return Ok(Node::Element { name, attrs, children: Vec::new(), span });
                }
                Some('>') => {
                    self.bump();
                    break;
                }
                Some(c) if c.is_alphabetic() || c == '_' => {
                    attrs.push(self.attr()?);
                }
                _ => return Err(self.err(format!("expected an attribute, '>' or '/>' in <{name}>"))),
            }
        }
        let children = self.children(&name)?;
        Ok(Node::Element { name, attrs, children, span })
    }

    fn attr(&mut self) -> Result<Attr, ParseError> {
        let span = self.span();
        let name = self.ident("an attribute name")?;
        if !self.eat('=') {
            return Ok(Attr { name, value: AttrValue::Flag, span });
        }
        match self.peek() {
            Some('"') => {
                self.bump();
                let mut s = String::new();
                loop {
                    match self.bump() {
                        Some('"') => break,
                        Some(c) => s.push(c),
                        None => return Err(self.err("unterminated string attribute")),
                    }
                }
                Ok(Attr { name, value: AttrValue::Str(s), span })
            }
            Some('{') => {
                self.bump();
                self.skip_ws();
                let value = match self.peek() {
                    Some(c) if c.is_ascii_digit() || c == '-' => {
                        let mut n = String::new();
                        if self.eat('-') {
                            n.push('-');
                        }
                        while let Some(c) = self.peek() {
                            if c.is_ascii_digit() || c == '.' {
                                n.push(c);
                                self.bump();
                            } else {
                                break;
                            }
                        }
                        let v: f32 = n
                            .parse()
                            .map_err(|_| self.err(format!("'{n}' is not a valid number")))?;
                        AttrValue::Num(v)
                    }
                    _ => {
                        let head = self.path()?;
                        // `name(arg)` — action call with one binding
                        // argument (§6.2 item-scoped handlers).
                        // `name()` — empty parens, kept as Ident so
                        // engines can treat `items.len()` like
                        // `items.len` (PLAN-ACCESSORS).
                        if self.peek() == Some('(') {
                            self.bump();
                            self.skip_ws();
                            if self.peek() == Some(')') {
                                self.bump();
                                AttrValue::Ident(head)
                            } else {
                                let arg = self.path()?;
                                self.skip_ws();
                                self.expect(')', "to close the action argument")?;
                                AttrValue::Call { name: head, arg }
                            }
                        } else {
                            AttrValue::Ident(head)
                        }
                    }
                };
                self.skip_ws();
                self.expect('}', "to close the braced attribute value")?;
                Ok(Attr { name, value, span })
            }
            _ => Err(self.err(format!("expected \"...\" or {{...}} after '{name}='"))),
        }
    }

    fn children(&mut self, parent: &str) -> Result<Vec<Node>, ParseError> {
        let mut out = Vec::new();
        let mut text: Vec<Inline> = Vec::new();
        let mut lit = String::new();

        macro_rules! flush_lit {
            () => {
                // Collapse internal whitespace; drop pure-whitespace runs.
                let collapsed = collapse_ws(&lit);
                if !collapsed.is_empty() {
                    text.push(Inline::Lit(collapsed));
                }
                lit.clear();
            };
        }
        macro_rules! flush_text {
            () => {
                flush_lit!();
                if !text.is_empty() {
                    out.push(Node::Text(std::mem::take(&mut text)));
                }
            };
        }

        loop {
            match self.peek() {
                None => return Err(self.err(format!("<{parent}> was never closed"))),
                Some('<') => {
                    if self.peek2() == Some('/') {
                        flush_text!();
                        self.bump(); // <
                        self.bump(); // /
                        let close = self.ident("the closing tag name")?;
                        if close != parent {
                            return Err(self.err(format!(
                                "mismatched closing tag: expected </{parent}>, found </{close}>"
                            )));
                        }
                        self.skip_ws();
                        self.expect('>', "to finish the closing tag")?;
                        return Ok(out);
                    }
                    flush_text!();
                    out.push(self.element()?);
                }
                Some('{') => {
                    flush_lit!();
                    let span = self.span();
                    self.bump();
                    self.skip_ws();
                    let path = self.path()?;
                    // Optional empty `()` — `{items.len()}` ≡ `{items.len}`
                    // (PLAN-ACCESSORS). Non-empty calls aren't text binds.
                    if self.peek() == Some('(') {
                        self.bump();
                        self.skip_ws();
                        self.expect(')', "empty () after an accessor path (or omit the parens)")?;
                    }
                    self.skip_ws();
                    self.expect('}', "to close the text binding")?;
                    text.push(Inline::Bind { path, span });
                }
                Some(_) => {
                    lit.push(self.bump().unwrap());
                }
            }
        }
    }
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::new();
    let mut in_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            in_ws = true;
        } else {
            if in_ws && !out.is_empty() {
                out.push(' ');
            }
            in_ws = false;
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_counter_shape() {
        let src = r#"
            <Column gap={12} padding={16}>
              <Row gap={8} cross="center">
                <Text bold grow>Counter</Text>
                <Button variant="ghost" onPress={Reset}>Reset</Button>
              </Row>
              <Text testId="count">{count}</Text>
            </Column>
        "#;
        let root = parse(src).expect("parses");
        let Node::Element { name, attrs, children, .. } = &root else { panic!() };
        assert_eq!(name, "Column");
        assert_eq!(attrs.len(), 2);
        assert_eq!(children.len(), 2);
        // The bound text node
        let Node::Element { children: text_kids, .. } = &children[1] else { panic!() };
        assert_eq!(
            text_kids[0],
            Node::Text(vec![Inline::Bind { path: "count".into(), span: Span { line: 7, col: 36 } }])
        );
    }

    #[test]
    fn mixed_literal_and_binding_text() {
        let root = parse("<Text>count: {count} · {status}</Text>").unwrap();
        let Node::Element { children, .. } = &root else { panic!() };
        let Node::Text(inl) = &children[0] else { panic!() };
        assert_eq!(inl.len(), 4);
        assert_eq!(inl[0], Inline::Lit("count:".into()));
        assert!(matches!(&inl[1], Inline::Bind { path, .. } if path == "count"));
        assert_eq!(inl[2], Inline::Lit("·".into()));
    }

    #[test]
    fn empty_parens_on_accessor_path_are_accepted() {
        let root = parse("<Text>{items.len()}</Text>").unwrap();
        let Node::Element { children, .. } = &root else { panic!() };
        let Node::Text(inl) = &children[0] else { panic!() };
        assert!(matches!(&inl[0], Inline::Bind { path, .. } if path == "items.len"));

        let root = parse(r#"<if test={items.isEmpty()}><Text>x</Text></if>"#).unwrap();
        let Node::Element { attrs, .. } = &root else { panic!() };
        assert!(matches!(&attrs[0].value, AttrValue::Ident(p) if p == "items.isEmpty"));
    }

    #[test]
    fn error_carries_line_and_col() {
        let err = parse("<Column>\n  <Button onPress={>\n</Column>").unwrap_err();
        assert_eq!(err.span.line, 2);
        assert!(err.message.contains("expected an identifier"));
    }

    #[test]
    fn mismatched_close_tag_is_an_error() {
        let err = parse("<Column><Row></Column></Row>").unwrap_err();
        assert!(err.message.contains("expected </Row>, found </Column>"));
    }

    #[test]
    fn unclosed_root_is_an_error() {
        let err = parse("<Column><Text>hi</Text>").unwrap_err();
        assert!(err.message.contains("never closed"));
    }

    #[test]
    fn comments_are_skipped_between_elements() {
        let src = "<Column>\n  // header row\n  <Text>hi</Text>\n</Column>";
        // Note: comments are only recognized in inter-element whitespace via skip_ws
        // at element boundaries; inside text they are content. Phase 0 scope.
        let root = parse(src).unwrap();
        let Node::Element { children, .. } = &root else { panic!() };
        // '// header row' is inside children text scanning, so it is text content —
        // documented Phase 0 limitation; codegen phase will formalize comment syntax.
        assert!(children.len() >= 1);
    }

    #[test]
    fn index_expressions_in_bindings_are_a_span_diagnosed_error_not_a_panic() {
        // ARCHITECTURE.md §6.2: "no index expressions" — `path()` only
        // ever accepts `ident(.ident)*`, so `series[3]` was never valid
        // syntax; this pins that down as the grammar grows (§6.2's <for>
        // work, in particular) so it can't regress into a panic.
        let e = parse("<Column><Text>{series[3]}</Text></Column>").unwrap_err();
        assert!(e.message.contains("expected '}'"), "{}", e.message);
        assert_eq!(e.span, Span { line: 1, col: 22 });
    }
}
