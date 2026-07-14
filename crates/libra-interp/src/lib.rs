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
    /// Resolve an iterable binding path to each element's display
    /// text (§6.2's `<for each={path}>`). `None` means the top-level
    /// resolve() call site doesn't support iteration (e.g. hand-built
    /// scopes in tests that never construct a `<for>`) — every real
    /// Scope built from `ReflectState::collection` supplies this.
    pub collection: &'a dyn Fn(&str) -> Option<Vec<String>>,
    /// Does this name resolve to a `FrameInfo`-payload handler (§12.5's
    /// `<Frames onTick={name}>`)? See `ReflectAction::action_with_frame_info`.
    pub has_frame_info_action: &'a dyn Fn(&str) -> bool,
}

pub fn resolve(ast: &Node, scope: &Scope) -> Result<ViewNode, InterpError> {
    element(ast, scope)
}

/// Resolve a list of children into a flat list of ViewNodes — `<for>`
/// expands to zero or more siblings, everything else to exactly one.
/// Bare text between elements is an error here (matches Column/Row's
/// existing contract: text must be wrapped in `<Text>`); `parent_name`
/// and `parent_span` are only for that error message.
fn resolve_children(
    children: &[Node],
    scope: &Scope,
    parent_name: &str,
    parent_span: Span,
) -> Result<Vec<ViewNode>, InterpError> {
    if children.iter().any(|c| matches!(c, Node::Text(_))) {
        return Err(err(
            parent_span,
            format!("<{parent_name}> cannot contain bare text; wrap it in <Text>"),
        ));
    }
    let mut out = Vec::new();
    for c in children {
        match c {
            Node::Element { name, .. } if name == "for" => out.extend(resolve_for(c, scope)?),
            _ => out.push(element(c, scope)?),
        }
    }
    Ok(out)
}

/// `<for each={path} as="name" key={...}>` (§6.2): iterate a
/// `Vec`-typed state field, binding `name` in the body to each
/// element's display text — the same Display/FromStr round-trip a
/// scalar binding already uses (works today for anything `Chart`/
/// `Sparkline` already round-trip via `SeriesHandle`).
///
/// v1 scope: `key` must be the loop variable itself (`key={name}`),
/// not a dot-path into it (`key={name.id}`) — there's no reconciler
/// consuming identity yet (CLAUDE.md: full tree rebuild per dispatch),
/// and dot-path access into a loop-bound Display string would need
/// structured (non-Display) reflection this doesn't have. For a type
/// like `SeriesHandle` whose Display already embeds its id, `key={name}`
/// is already a unique identity per element.
fn resolve_for(node: &Node, scope: &Scope) -> Result<Vec<ViewNode>, InterpError> {
    let Node::Element { attrs, children, span, .. } = node else { unreachable!() };

    let mut each: Option<(String, Span)> = None;
    let mut as_name: Option<String> = None;
    let mut key: Option<(String, Span)> = None;
    for a in attrs {
        match (a.name.as_str(), &a.value) {
            ("each", AttrValue::Ident(path)) => each = Some((path.clone(), a.span)),
            ("as", AttrValue::Str(s)) => as_name = Some(s.clone()),
            ("key", AttrValue::Ident(k)) => key = Some((k.clone(), a.span)),
            _ => return Err(unknown_attr("for", a)),
        }
    }
    let (path, espan) = each.ok_or_else(|| err(*span, "<for> requires each={someCollection}"))?;
    let as_name =
        as_name.ok_or_else(|| err(*span, "<for> requires as=\"name\" (the loop variable)"))?;
    let (key_path, kspan) = key
        .ok_or_else(|| err(*span, "<for> requires key={...} — identity for reordering (§6.2)"))?;
    if key_path != as_name {
        return Err(err(
            kspan,
            format!(
                "<for>: key={{{key_path}}} — only key={{{as_name}}} (the loop variable itself) is \
                 supported today; per-field keys into loop-bound values aren't implemented yet"
            ),
        ));
    }

    let items = (scope.collection)(&path)
        .ok_or_else(|| err(espan, format!("no collection '{path}' on State")))?;

    let mut out = Vec::new();
    for item in &items {
        let item_field = |p: &str| -> Option<String> {
            if p == as_name {
                Some(item.clone())
            } else {
                (scope.field)(p)
            }
        };
        let iter_scope = Scope {
            field: &item_field,
            has_action: scope.has_action,
            has_str_action: scope.has_str_action,
            collection: scope.collection,
            has_frame_info_action: scope.has_frame_info_action,
        };
        out.extend(resolve_children(children, &iter_scope, "for", *span)?);
    }
    Ok(out)
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
            let kids = resolve_children(children, scope, name, *span)?;
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

        // §12.5: invisible, zero-layout — declares a mount-scoped
        // subscription rather than rendering anything. `Interval`/
        // `Stream` are the same pattern, not yet implemented.
        "Frames" => {
            let mut on_tick: Option<(String, Span)> = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("onTick", AttrValue::Ident(m)) => on_tick = Some((m.clone(), a.span)),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let (msg, mspan) =
                on_tick.ok_or_else(|| err(*span, "<Frames> requires onTick={someAction}"))?;
            if !(scope.has_frame_info_action)(&msg) {
                return Err(err(
                    mspan,
                    format!("'{msg}' is not a FrameInfo-payload Action variant (need `{msg}(FrameInfo)`)"),
                ));
            }
            Ok(ViewNode::SourceFrames { on_tick: msg })
        }

        "for" => Err(err(*span, "<for> is only allowed as a direct child of <Column>/<Row>")),

        other => Err(err(
            *span,
            format!(
                "unknown element <{other}> — available: Column, Row, Text, Button, Chart, Sparkline, Input, Frames, for"
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
        Scope {
            field,
            has_action,
            has_str_action: &|_| false,
            collection: &|_| None,
            has_frame_info_action: &|_| false,
        }
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

    fn scope_with_collection<'a>(
        field: &'a dyn Fn(&str) -> Option<String>,
        collection: &'a dyn Fn(&str) -> Option<Vec<String>>,
    ) -> Scope<'a> {
        Scope {
            field,
            has_action: &|_| true,
            has_str_action: &|_| false,
            collection,
            has_frame_info_action: &|_| false,
        }
    }

    #[test]
    fn for_loop_expands_into_one_node_per_item_bound_to_the_loop_variable() {
        let ast = parse(
            r#"<Column><for each={items} as="x" key={x}><Text>{x}</Text></for></Column>"#,
        )
        .unwrap();
        let field = |_: &str| None;
        let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let collection = move |p: &str| (p == "items").then(|| items.clone());
        let view = resolve(&ast, &scope_with_collection(&field, &collection)).unwrap();
        let ViewNode::Column { children, .. } = view else { panic!() };
        assert_eq!(children.len(), 3);
        for (i, expected) in ["a", "b", "c"].iter().enumerate() {
            let ViewNode::Text { content, .. } = &children[i] else { panic!() };
            assert_eq!(content, expected);
        }
    }

    #[test]
    fn for_loop_body_still_resolves_outer_state_fields() {
        // §6.2's join rule (unrelated to <for>) spaces adjacent inline
        // parts, hence "a - 7" not "a-7" — this test's actual point is
        // that {count} still resolves via the outer scope while {x}
        // resolves via the loop-local one, in the same Text.
        let ast = parse(
            r#"<Column><for each={items} as="x" key={x}><Text>{x}-{count}</Text></for></Column>"#,
        )
        .unwrap();
        let field = |p: &str| (p == "count").then(|| "7".to_string());
        let items = vec!["a".to_string()];
        let collection = move |p: &str| (p == "items").then(|| items.clone());
        let view = resolve(&ast, &scope_with_collection(&field, &collection)).unwrap();
        let ViewNode::Column { children, .. } = view else { panic!() };
        let ViewNode::Text { content, .. } = &children[0] else { panic!() };
        assert_eq!(content, "a - 7");
    }

    #[test]
    fn for_requires_each() {
        let ast = parse(r#"<Column><for as="x" key={x}><Text>{x}</Text></for></Column>"#).unwrap();
        let field = |_: &str| None;
        let collection = |_: &str| None;
        let e = resolve(&ast, &scope_with_collection(&field, &collection)).unwrap_err();
        assert!(e.message.contains("requires each="));
    }

    #[test]
    fn for_requires_key() {
        let ast = parse(r#"<Column><for each={items} as="x"><Text>{x}</Text></for></Column>"#).unwrap();
        let field = |_: &str| None;
        let items = vec!["a".to_string()];
        let collection = move |p: &str| (p == "items").then(|| items.clone());
        let e = resolve(&ast, &scope_with_collection(&field, &collection)).unwrap_err();
        assert!(e.message.contains("requires key="));
    }

    #[test]
    fn for_key_must_be_the_loop_variable_itself() {
        let ast = parse(
            r#"<Column><for each={items} as="x" key={x.id}><Text>{x}</Text></for></Column>"#,
        )
        .unwrap();
        let field = |_: &str| None;
        let items = vec!["a".to_string()];
        let collection = move |p: &str| (p == "items").then(|| items.clone());
        let e = resolve(&ast, &scope_with_collection(&field, &collection)).unwrap_err();
        assert!(e.message.contains("only key={x}"), "{}", e.message);
    }

    #[test]
    fn for_names_a_real_collection_or_it_is_a_load_time_error() {
        let ast = parse(
            r#"<Column><for each={nonexistent} as="x" key={x}><Text>{x}</Text></for></Column>"#,
        )
        .unwrap();
        let field = |_: &str| None;
        let collection = |_: &str| None;
        let e = resolve(&ast, &scope_with_collection(&field, &collection)).unwrap_err();
        assert!(e.message.contains("no collection 'nonexistent' on State"));
    }

    fn scope_with_frame_info<'a>(has_frame_info_action: &'a dyn Fn(&str) -> bool) -> Scope<'a> {
        Scope {
            field: &|_| None,
            has_action: &|_| true,
            has_str_action: &|_| false,
            collection: &|_| None,
            has_frame_info_action,
        }
    }

    #[test]
    fn frames_resolves_to_a_source_node_when_on_tick_names_a_real_handler() {
        let ast = parse(r#"<Column><Frames onTick={frame} /></Column>"#).unwrap();
        let has_frame_info_action = |m: &str| m == "frame";
        let view = resolve(&ast, &scope_with_frame_info(&has_frame_info_action)).unwrap();
        let ViewNode::Column { children, .. } = view else { panic!() };
        assert_eq!(children, vec![ViewNode::SourceFrames { on_tick: "frame".into() }]);
    }

    #[test]
    fn frames_requires_on_tick() {
        let ast = parse(r#"<Column><Frames /></Column>"#).unwrap();
        let has_frame_info_action = |_: &str| true;
        let e = resolve(&ast, &scope_with_frame_info(&has_frame_info_action)).unwrap_err();
        assert!(e.message.contains("requires onTick="));
    }

    #[test]
    fn frames_on_tick_must_name_a_frame_info_handler() {
        let ast = parse(r#"<Column><Frames onTick={notAHandler} /></Column>"#).unwrap();
        let has_frame_info_action = |_: &str| false;
        let e = resolve(&ast, &scope_with_frame_info(&has_frame_info_action)).unwrap_err();
        assert!(e.message.contains("not a FrameInfo-payload Action variant"), "{}", e.message);
    }

    #[test]
    fn frames_is_invisible_inside_a_for_loop_too() {
        // Mounted sources inside <for> bodies are exactly as valid as
        // top-level ones — resolve_children handles both uniformly.
        let ast = parse(
            r#"<Column><for each={items} as="x" key={x}><Frames onTick={frame} /></for></Column>"#,
        )
        .unwrap();
        let field = |_: &str| None;
        let items = vec!["a".to_string(), "b".to_string()];
        let collection = move |p: &str| (p == "items").then(|| items.clone());
        let has_frame_info_action = |m: &str| m == "frame";
        let scope = Scope {
            field: &field,
            has_action: &|_| true,
            has_str_action: &|_| false,
            collection: &collection,
            has_frame_info_action: &has_frame_info_action,
        };
        let view = resolve(&ast, &scope).unwrap();
        let ViewNode::Column { children, .. } = view else { panic!() };
        assert_eq!(children.len(), 2);
        for child in &children {
            assert_eq!(child, &ViewNode::SourceFrames { on_tick: "frame".into() });
        }
    }
}
