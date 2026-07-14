//! libra-interp — the dev engine: walk a parsed `.lx` AST against the
//! live state's reflection to produce a ViewTree (ARCHITECTURE.md §6.4).
//!
//! Validation happens here, at load/rebuild time, with spans:
//! unknown elements, unknown attributes, bindings that name no state
//! field, and handlers that name no action are all errors — the
//! same diagnostics the release codegen turns into compile errors.

use libra_markup::{Attr, AttrValue, Inline, Node, Span};
use libra_components::{BoxStyle, TextSize, Variant, ViewNode};

#[derive(Debug, Clone)]
pub struct InterpError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for InterpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.span, self.message)
    }
}
impl std::error::Error for InterpError {}

fn err(span: Span, msg: impl Into<String>) -> InterpError {
    InterpError { message: msg.into(), span }
}

/// State/Action reflection the interpreter resolves against
/// (`#[derive(LibraReflect)]` on the app side).
pub struct Scope<'a> {
    pub field: &'a dyn Fn(&str) -> Option<String>,
    pub has_action: &'a dyn Fn(&str) -> bool,
    /// Does this name resolve to a String-payload handler
    /// (`onChange={TitleChanged}`)? See `ReflectAction::action_with_str`.
    pub has_str_action: &'a dyn Fn(&str) -> bool,
}

pub fn resolve(ast: &Node, scope: &Scope) -> Result<ViewNode, InterpError> {
    element(ast, scope)
}

fn element(node: &Node, scope: &Scope) -> Result<ViewNode, InterpError> {
    let Node::Element { name, attrs, children, span } = node else {
        return Err(err(
            Span { line: 0, col: 0 },
            "text content is only allowed inside <Text> and <Button>",
        ));
    };

    match name.as_str() {
        "Column" | "Row" => {
            let mut style = BoxStyle::default();
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("gap", AttrValue::Num(v)) => style.gap = *v,
                    ("padding", AttrValue::Num(v)) => style.padding = *v,
                    ("grow", AttrValue::Flag) => style.grow = true,
                    ("cross", AttrValue::Str(s)) if s == "center" => style.cross_center = true,
                    ("main", AttrValue::Str(s)) if s == "center" => style.main_center = true,
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let kids = children
                .iter()
                .filter(|c| !matches!(c, Node::Text(_)))
                .map(|c| element(c, scope))
                .collect::<Result<Vec<_>, _>>()?;
            if children.iter().any(|c| matches!(c, Node::Text(_))) {
                return Err(err(*span, format!("<{name}> cannot contain bare text; wrap it in <Text>")));
            }
            Ok(if name == "Column" {
                ViewNode::Column { style, children: kids }
            } else {
                ViewNode::Row { style, children: kids }
            })
        }

        "Text" => {
            let mut size = TextSize::Md;
            let mut bold = false;
            let mut muted = false;
            let mut grow = false;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("size", AttrValue::Str(s)) => {
                        size = match s.as_str() {
                            "sm" => TextSize::Sm,
                            "md" => TextSize::Md,
                            "xl" => TextSize::Xl,
                            other => {
                                return Err(err(
                                    a.span,
                                    format!("unknown size \"{other}\" — expected \"sm\", \"md\" or \"xl\""),
                                ))
                            }
                        }
                    }
                    ("bold", AttrValue::Flag) => bold = true,
                    ("muted", AttrValue::Flag) => muted = true,
                    ("grow", AttrValue::Flag) => grow = true,
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let content = text_content(children, scope, *span)?;
            Ok(ViewNode::Text { content, size, bold, muted, grow, test_id })
        }

        "Button" => {
            let mut variant = Variant::Secondary;
            let mut on_press: Option<(String, Span)> = None;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("variant", AttrValue::Str(s)) => {
                        variant = match s.as_str() {
                            "primary" => Variant::Primary,
                            "secondary" => Variant::Secondary,
                            "ghost" => Variant::Ghost,
                            other => {
                                return Err(err(
                                    a.span,
                                    format!("unknown variant \"{other}\" — expected \"primary\", \"secondary\" or \"ghost\""),
                                ))
                            }
                        }
                    }
                    ("onPress", AttrValue::Ident(m)) => on_press = Some((m.clone(), a.span)),
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let (msg, mspan) =
                on_press.ok_or_else(|| err(*span, "<Button> requires onPress={someAction}"))?;
            if !(scope.has_action)(&msg) {
                return Err(err(mspan, format!("no action '{msg}' on Action — check the enum")));
            }
            let label = text_content(children, scope, *span)?;
            Ok(ViewNode::Button { label, variant, on_press: msg, test_id })
        }

        "Chart" | "Sparkline" => {
            let mut handle: Option<(String, Span)> = None;
            let mut width = 60.0;
            let mut height = if name == "Chart" { 120.0 } else { 20.0 };
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("series", AttrValue::Ident(path)) => handle = Some((path.clone(), a.span)),
                    ("height", AttrValue::Num(v)) => height = *v,
                    ("width", AttrValue::Num(v)) if name == "Sparkline" => width = *v,
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let (path, hspan) = handle
                .ok_or_else(|| err(*span, format!("<{name}> requires series={{someHandle}}")))?;
            let text = (scope.field)(&path)
                .ok_or_else(|| err(hspan, format!("no field '{path}' on State")))?;
            let handle: libra_core::SeriesHandle = text.parse().map_err(|e| {
                err(hspan, format!("field '{path}' is not a series handle: {e}"))
            })?;
            Ok(if name == "Chart" {
                ViewNode::Chart { handle, height, test_id }
            } else {
                ViewNode::Sparkline { handle, width, height, test_id }
            })
        }

        "Input" => {
            let mut value: Option<(String, Span)> = None;
            let mut placeholder = String::new();
            let mut on_change: Option<(String, Span)> = None;
            let mut test_id: Option<String> = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("value", AttrValue::Ident(path)) => value = Some((path.clone(), a.span)),
                    ("placeholder", AttrValue::Str(s)) => placeholder = s.clone(),
                    ("onChange", AttrValue::Ident(m)) => on_change = Some((m.clone(), a.span)),
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let (path, vspan) =
                value.ok_or_else(|| err(*span, "<Input> requires value={someField}"))?;
            let value = (scope.field)(&path)
                .ok_or_else(|| err(vspan, format!("no field '{path}' on State")))?;
            let (msg, mspan) =
                on_change.ok_or_else(|| err(*span, "<Input> requires onChange={someAction}"))?;
            if !(scope.has_str_action)(&msg) {
                return Err(err(
                    mspan,
                    format!("'{msg}' is not a String-payload Action variant (need `{msg}(String)`)"),
                ));
            }
            // Focus identity until structural IDs arrive with the
            // reconciler — required, load-time enforced.
            let test_id = test_id
                .ok_or_else(|| err(*span, "<Input> requires testId=\"…\" (focus identity)"))?;
            Ok(ViewNode::Input { value, placeholder, on_change: msg, test_id })
        }

        other => Err(err(
            *span,
            format!(
                "unknown element <{other}> — available: Column, Row, Text, Button, Chart, Sparkline, Input"
            ),
        )),
    }
}

fn unknown_attr(el: &str, a: &Attr) -> InterpError {
    err(a.span, format!("unknown or invalid attribute '{}' on <{el}>", a.name))
}

fn text_content(children: &[Node], scope: &Scope, span: Span) -> Result<String, InterpError> {
    let mut out = String::new();
    for (i, child) in children.iter().enumerate() {
        let Node::Text(inlines) = child else {
            return Err(err(span, "nested elements inside text content are not allowed in Phase 0"));
        };
        for (j, inline) in inlines.iter().enumerate() {
            if (i, j) != (0, 0) && !out.is_empty() {
                // Inline parts were whitespace-separated in source.
                if !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            match inline {
                Inline::Lit(s) => out.push_str(s),
                Inline::Bind { path, span } => {
                    let v = (scope.field)(path)
                        .ok_or_else(|| err(*span, format!("no field '{path}' on State")))?;
                    out.push_str(&v);
                }
            }
        }
    }
    // A lone binding like {count} should render exactly its value (no pad).
    Ok(out.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use libra_markup::parse;

    fn scope_ok<'a>(
        field: &'a dyn Fn(&str) -> Option<String>,
        has_action: &'a dyn Fn(&str) -> bool,
    ) -> Scope<'a> {
        Scope { field, has_action, has_str_action: &|_| false }
    }

    #[test]
    fn resolves_counter_view() {
        let ast = parse(
            r#"<Column gap={12}>
                 <Text testId="count">{count}</Text>
                 <Button variant="primary" onPress={Increment}>+</Button>
               </Column>"#,
        )
        .unwrap();
        let field = |p: &str| (p == "count").then(|| "7".to_string());
        let has_action = |m: &str| m == "Increment";
        let view = resolve(&ast, &scope_ok(&field, &has_action)).unwrap();
        let ViewNode::Column { children, .. } = view else { panic!() };
        assert_eq!(
            children[0],
            ViewNode::Text {
                content: "7".into(),
                size: TextSize::Md,
                bold: false,
                muted: false,
                grow: false,
                test_id: Some("count".into())
            }
        );
    }

    #[test]
    fn typoed_handler_is_a_load_time_error_with_span() {
        let ast = parse("<Column>\n  <Button onPress={Incremnt}>+</Button>\n</Column>").unwrap();
        let field = |_: &str| None;
        let has_action = |m: &str| m == "Increment";
        let e = resolve(&ast, &scope_ok(&field, &has_action)).unwrap_err();
        assert!(e.message.contains("no action 'Incremnt' on Action"));
        assert_eq!(e.span.line, 2);
    }

    #[test]
    fn typoed_binding_is_a_load_time_error() {
        let ast = parse("<Column><Text>{cuont}</Text></Column>").unwrap();
        let field = |p: &str| (p == "count").then(|| "0".into());
        let has_action = |_: &str| true;
        let e = resolve(&ast, &scope_ok(&field, &has_action)).unwrap_err();
        assert!(e.message.contains("no field 'cuont' on State"));
    }

    #[test]
    fn unknown_element_is_an_error() {
        let ast = parse("<Colunm></Colunm>").unwrap();
        let field = |_: &str| None;
        let has_action = |_: &str| true;
        let e = resolve(&ast, &scope_ok(&field, &has_action)).unwrap_err();
        assert!(e.message.contains("unknown element <Colunm>"));
    }
}
