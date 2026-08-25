//! libra-interp — the dev view engine: walk a parsed `.lx` AST against the
//! live state's reflection to produce a ViewTree. Validation happens here at
//! load/rebuild time, with spans (unknown elements/attrs, bindings or
//! handlers that name nothing) — the same diagnostics codegen turns into
//! compile errors.
//!
//! Architecture: ARCHITECTURE.md §6.4 (one grammar, two engines).

use libra_markup::{
    is_catalog_element, Attr, AttrValue, ComponentDef, ComponentRegistry, Inline, Node, PropParam,
    PropType, Span,
};
use libra_components::{BoxStyle, DrawerSide, TextSize, Tone, Variant, ViewNode};
use libra_core::CollectionRow;
use std::collections::{HashMap, HashSet};

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

/// §3.3's grammar-level rule, mirrored from libra-codegen so both
/// engines reject the same markup identically (§6.4): a handler
/// reference is camelCase or it's a style error, not a silently
/// accepted alternate spelling. PascalCase reads as a component
/// reference in JSX-flavored markup — `onPress={Reset}` (the
/// pre-revision style) is rejected before any ReflectAction lookup,
/// with a suggestion naming the camelCase form.
fn require_camel_case(name: &str, span: Span) -> Result<(), InterpError> {
    if name.chars().next().is_some_and(|c| c.is_uppercase()) {
        return Err(err(
            span,
            format!(
                "handler name '{name}' must be camelCase (§3.3) — did you mean '{}'?",
                to_camel_case(name)
            ),
        ));
    }
    Ok(())
}

fn to_camel_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// State/Action reflection the interpreter resolves against
/// (`#[derive(LibraReflect)]` on the app side).
pub struct Scope<'a> {
    pub field: &'a dyn Fn(&str) -> Option<String>,
    pub has_action: &'a dyn Fn(&str) -> bool,
    /// Does this name resolve to a String-payload handler
    /// (`onChange={titleChanged}`)? See `ReflectAction::action_with_str`.
    pub has_str_action: &'a dyn Fn(&str) -> bool,
    /// Resolve an iterable binding path to structured rows (§6.2's
    /// `<for each={path}>`, including item fields). `None` means the
    /// top-level resolve() call site doesn't support iteration (e.g.
    /// hand-built scopes in tests that never construct a `<for>`) —
    /// every real Scope built from `ReflectState::collection_rows`
    /// supplies this.
    pub collection_rows: &'a dyn Fn(&str) -> Option<Vec<CollectionRow>>,
    /// Does this name resolve to a `FrameInfo`-payload handler (§12.5's
    /// `<Frames onTick={name}>`)? See `ReflectAction::action_with_frame_info`.
    pub has_frame_info_action: &'a dyn Fn(&str) -> bool,
    /// Does this name resolve to an `Instant`-payload handler (§12.5's
    /// `<Interval every={ms} onTick={name}>`)? See
    /// `ReflectAction::action_with_instant`.
    pub has_instant_action: &'a dyn Fn(&str) -> bool,
    /// `"State"` or `"props"` — controls binding-not-found diagnostics (§7.2).
    pub binding_ctx: &'static str,
}

/// Resolve a view tree. Pass an empty [`ComponentRegistry`] when no composed
/// components are in scope.
pub fn resolve(ast: &Node, scope: &Scope, registry: &ComponentRegistry) -> Result<ViewNode, InterpError> {
    element(ast, scope, registry, None)
}

/// Resolve a list of children into a flat list of ViewNodes — `<for>`
/// and `<if>` expand to zero or more siblings, everything else to
/// exactly one. Bare text between elements is an error here (matches
/// Column/Row's existing contract: text must be wrapped in `<Text>`);
/// `parent_name` and `parent_span` are only for that error message.
fn resolve_children(
    children: &[Node],
    scope: &Scope,
    registry: &ComponentRegistry,
    slot_children: Option<&[Node]>,
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
            Node::Element { name, .. } if name == "for" => {
                out.extend(resolve_for(c, scope, registry, slot_children)?)
            }
            Node::Element { name, .. } if name == "if" => {
                out.extend(resolve_if(c, scope, registry, slot_children)?)
            }
            _ => out.extend(resolve_nodes(c, scope, registry, slot_children)?),
        }
    }
    Ok(out)
}

/// `<for each={path} as="name" key={...}>` (§6.2): iterate a
/// `Vec`-typed state field. Opaque elements bind `name` to the item's
/// Display text; structured elements (`ReflectState` item types) also
/// expose `{name.field}` for binds, keys, and item-scoped handler args
/// (PLAN-ITEM-FIELDS).
///
/// `key` is the loop variable itself (`key={name}`) or one field on it
/// (`key={name.id}`). Nested paths (`name.a.b`) and keys that don't
/// name the loop variable are span-diagnosed errors. There's no
/// reconciler consuming identity yet (full tree rebuild per dispatch);
/// keys are identity *data* for later.
fn resolve_for(
    node: &Node,
    scope: &Scope,
    registry: &ComponentRegistry,
    slot_children: Option<&[Node]>,
) -> Result<Vec<ViewNode>, InterpError> {
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
    validate_loop_key(&key_path, &as_name, kspan)?;

    let items = (scope.collection_rows)(&path)
        .ok_or_else(|| err(espan, format!("no collection '{path}' on State")))?;

    let mut out = Vec::new();
    for row in &items {
        let as_name = as_name.as_str();
        let item_field = |p: &str| -> Option<String> {
            if let Some(v) = row.get(p, as_name) {
                return Some(v);
            }
            // A failed loop-scoped path must not fall through to outer
            // state (would hide typos like `{t.tex}` as a state lookup).
            if p == as_name || p.starts_with(&format!("{as_name}.")) {
                return None;
            }
            (scope.field)(p)
        };
        let iter_scope = Scope {
            field: &item_field,
            has_action: scope.has_action,
            has_str_action: scope.has_str_action,
            collection_rows: scope.collection_rows,
            has_frame_info_action: scope.has_frame_info_action,
            has_instant_action: scope.has_instant_action,
            binding_ctx: scope.binding_ctx,
        };
        out.extend(resolve_children(
            children,
            &iter_scope,
            registry,
            slot_children,
            "for",
            *span,
        )?);
    }
    Ok(out)
}

/// `key={as}` or `key={as.field}` — one field segment only.
fn validate_loop_key(key_path: &str, as_name: &str, span: Span) -> Result<(), InterpError> {
    if key_path == as_name {
        return Ok(());
    }
    let Some(field) = key_path.strip_prefix(&format!("{as_name}.")) else {
        return Err(err(
            span,
            format!(
                "<for>: key={{{key_path}}} — key must be {{{as_name}}} or {{{as_name}.field}}"
            ),
        ));
    };
    if field.is_empty() || field.contains('.') {
        return Err(err(
            span,
            format!(
                "<for>: key={{{key_path}}} — only one field segment is supported \
                 (got {{{as_name}.field}}, not nested paths)"
            ),
        ));
    }
    Ok(())
}

/// `<if test={flag}>…<else>…</else></if>` (§6.2 / PLAN-IF + PLAN-ACCESSORS):
/// expand to the then-branch or else-branch children. `test` is any
/// binding whose Display equals `"true"` (bool fields, `isEmpty`
/// accessors). The If node itself never appears in the ViewTree.
fn resolve_if(
    node: &Node,
    scope: &Scope,
    registry: &ComponentRegistry,
    slot_children: Option<&[Node]>,
) -> Result<Vec<ViewNode>, InterpError> {
    let Node::Element { attrs, children, span, .. } = node else { unreachable!() };

    let mut test: Option<(String, Span)> = None;
    for a in attrs {
        match (a.name.as_str(), &a.value) {
            ("test", AttrValue::Ident(path)) => test = Some((path.clone(), a.span)),
            _ => return Err(unknown_attr("if", a)),
        }
    }
    let (path, tspan) = test.ok_or_else(|| err(*span, "<if> requires test={someBoolField}"))?;
    let (then_kids, else_kids) = split_if_branches(children, *span)?;
    let value = (scope.field)(&path)
        .ok_or_else(|| err(tspan, format!("no field '{path}' on State")))?;
    let take_then = value == "true";
    let branch = if take_then { then_kids } else { else_kids };
    resolve_children(branch, scope, registry, slot_children, "if", *span)
}

/// Resolved prop values at a component call site (§7.2).
struct PropBindings {
    display: HashMap<String, String>,
    unit_actions: HashSet<String>,
    str_actions: HashSet<String>,
    slot_children: Option<Vec<Node>>,
    prop_names: HashSet<String>,
}

fn resolve_unit_handler(scope: &Scope, name: &str, span: Span) -> Result<String, InterpError> {
    if scope.binding_ctx == "props" {
        return (scope.field)(name).ok_or_else(|| {
            err(span, format!("no prop '{name}' — components resolve handlers only against props (§7.2)"))
        });
    }
    require_camel_case(name, span)?;
    if !(scope.has_action)(name) {
        return Err(err(span, format!("no action '{name}' on Action — check the enum")));
    }
    Ok(name.to_string())
}

fn expand_component_call(
    comp_name: &str,
    attrs: &[Attr],
    call_children: &[Node],
    def: &ComponentDef,
    outer: &Scope,
    registry: &ComponentRegistry,
    call_span: Span,
) -> Result<Vec<ViewNode>, InterpError> {
    let PropBindings {
        display,
        unit_actions,
        str_actions,
        slot_children,
        prop_names,
    } = bind_call_props(comp_name, attrs, call_children, def, outer, call_span)?;
    let display = display;
    let prop_names = prop_names;
    let unit_actions = unit_actions;
    let str_actions = str_actions;
    let field = move |p: &str| -> Option<String> {
        if let Some(v) = display.get(p) {
            return Some(v.clone());
        }
        let root = p.split('.').next().unwrap_or(p);
        if prop_names.contains(root) {
            return None;
        }
        None
    };
    let has_action = move |m: &str| unit_actions.contains(m);
    let has_str_action = move |m: &str| str_actions.contains(m);
    let prop_scope = Scope {
        field: &field,
        has_action: &has_action,
        has_str_action: &has_str_action,
        collection_rows: &|_| None,
        has_frame_info_action: &|_| false,
        has_instant_action: &|_| false,
        binding_ctx: "props",
    };
    let mut out = Vec::new();
    for body in &def.body {
        out.extend(resolve_nodes(
            body,
            &prop_scope,
            registry,
            slot_children.as_deref(),
        )?);
    }
    Ok(out)
}

fn resolve_nodes(
    node: &Node,
    scope: &Scope,
    registry: &ComponentRegistry,
    slot_children: Option<&[Node]>,
) -> Result<Vec<ViewNode>, InterpError> {
    let Node::Element { name, attrs, children, span } = node else {
        return Err(err(
            Span { line: 0, col: 0 },
            "text content is only allowed inside <Text>, <Button>, and <Badge>",
        ));
    };
    if !is_catalog_element(name) {
        if let Some(def) = registry.get(name) {
            return expand_component_call(name, attrs, children, def, scope, registry, *span);
        }
        return Err(err(
            *span,
            format!("unknown component <{name}> — no component file defines it (§7.2)"),
        ));
    }
    Ok(vec![element(node, scope, registry, slot_children)?])
}

fn bind_call_props(
    comp_name: &str,
    attrs: &[Attr],
    call_children: &[Node],
    def: &ComponentDef,
    outer: &Scope,
    call_span: Span,
) -> Result<PropBindings, InterpError> {
    let mut bindings = PropBindings {
        display: HashMap::new(),
        unit_actions: HashSet::new(),
        str_actions: HashSet::new(),
        slot_children: None,
        prop_names: def.props.iter().map(|p| p.name.clone()).collect(),
    };
    let mut seen = HashSet::new();

    for param in &def.props {
        if param.ty == PropType::Slot {
            bindings.slot_children = Some(call_children.to_vec());
            seen.insert(param.name.clone());
            continue;
        }
        let Some(attr) = attrs.iter().find(|a| a.name == param.name) else {
            return Err(err(
                call_span,
                format!("<{comp_name}> missing required prop '{}'", param.name),
            ));
        };
        seen.insert(param.name.clone());
        bind_one_prop(comp_name, param, attr, outer, &mut bindings)?;
    }

    for a in attrs {
        if !seen.contains(&a.name) {
            let hint = suggest_prop_name(&a.name, &def.props);
            let msg = hint.map_or_else(
                || format!("no prop '{}' on {comp_name}", a.name),
                |s| format!("no prop '{}' on {comp_name} — did you mean '{s}'?", a.name),
            );
            return Err(err(a.span, msg));
        }
    }

    let has_slot = def.props.iter().any(|p| p.ty == PropType::Slot);
    if !has_slot && !call_children.is_empty() {
        return Err(err(
            call_span,
            format!("<{comp_name}> takes no children — pass content via a slot prop"),
        ));
    }
    Ok(bindings)
}

fn bind_one_prop(
    comp_name: &str,
    param: &PropParam,
    attr: &Attr,
    outer: &Scope,
    bindings: &mut PropBindings,
) -> Result<(), InterpError> {
    match param.ty {
        PropType::String => {
            let v = match &attr.value {
                AttrValue::Str(s) => s.clone(),
                AttrValue::Ident(path) => (outer.field)(path).ok_or_else(|| {
                    err(attr.span, format!("no field '{path}' on State"))
                })?,
                _ => {
                    return Err(err(
                        attr.span,
                        format!("prop '{}' on {comp_name} expects a string or {{binding}}", param.name),
                    ))
                }
            };
            bindings.display.insert(param.name.clone(), v);
        }
        PropType::Num => {
            let v = match &attr.value {
                AttrValue::Num(n) => n.to_string(),
                AttrValue::Ident(path) => {
                    let s = (outer.field)(path).ok_or_else(|| {
                        err(attr.span, format!("no field '{path}' on State"))
                    })?;
                    s.parse::<f32>().map_err(|_| {
                        err(
                            attr.span,
                            format!(
                                "prop '{}' on {comp_name} is Num but '{path}' displays as '{s}'",
                                param.name
                            ),
                        )
                    })?;
                    s
                }
                _ => {
                    return Err(err(
                        attr.span,
                        format!("prop '{}' on {comp_name} expects a number or {{binding}}", param.name),
                    ))
                }
            };
            bindings.display.insert(param.name.clone(), v);
        }
        PropType::Bool => {
            let v = match &attr.value {
                AttrValue::Flag => "true".to_string(),
                AttrValue::Ident(path) => {
                    let s = (outer.field)(path).ok_or_else(|| {
                        err(attr.span, format!("no field '{path}' on State"))
                    })?;
                    if s != "true" && s != "false" {
                        return Err(err(
                            attr.span,
                            format!("Bool prop '{}' expects Display \"true\" or \"false\"", param.name),
                        ));
                    }
                    s
                }
                _ => return Err(err(attr.span, "Bool prop expects a flag or {binding}")),
            };
            bindings.display.insert(param.name.clone(), v);
        }
        PropType::Action => {
            let AttrValue::Ident(name) = &attr.value else {
                return Err(err(attr.span, "Action prop expects {handlerName}"));
            };
            require_camel_case(name, attr.span)?;
            if !(outer.has_action)(name) {
                return Err(err(attr.span, format!("no action '{name}' on Action")));
            }
            bindings.unit_actions.insert(name.clone());
            bindings.display.insert(param.name.clone(), name.clone());
        }
        PropType::ActionString => {
            let name = match &attr.value {
                AttrValue::Ident(name) => {
                    require_camel_case(name, attr.span)?;
                    if !(outer.has_str_action)(name) {
                        return Err(err(
                            attr.span,
                            format!("'{name}' is not a String-payload Action variant"),
                        ));
                    }
                    name.clone()
                }
                AttrValue::Call { name, arg } => {
                    require_camel_case(name, attr.span)?;
                    if !(outer.has_str_action)(name) {
                        return Err(err(
                            attr.span,
                            format!("'{name}' is not a String-payload Action variant"),
                        ));
                    }
                    let _ = (outer.field)(arg).ok_or_else(|| err(attr.span, format!("no binding '{arg}'")))?;
                    name.clone()
                }
                _ => return Err(err(attr.span, "Action(String) prop expects {handler} or {handler(arg)}")),
            };
            bindings.str_actions.insert(name.clone());
            bindings.display.insert(param.name.clone(), name);
        }
        PropType::Slot => unreachable!(),
    }
    Ok(())
}

fn suggest_prop_name(name: &str, params: &[PropParam]) -> Option<String> {
    params
        .iter()
        .map(|p| p.name.as_str())
        .min_by_key(|n| levenshtein(n, name))
        .filter(|n| levenshtein(n, name) <= 2)
        .map(str::to_string)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Partition `<if>` children into then / else. At most one `<else>`;
/// it may only appear as a direct child and carries no attributes.
fn split_if_branches<'a>(
    children: &'a [Node],
    span: Span,
) -> Result<(&'a [Node], &'a [Node]), InterpError> {
    let mut else_at: Option<usize> = None;
    for (i, c) in children.iter().enumerate() {
        if let Node::Element { name, .. } = c {
            if name == "else" {
                if else_at.is_some() {
                    return Err(err(span, "<if> allows at most one <else>"));
                }
                else_at = Some(i);
            }
        }
    }
    match else_at {
        None => Ok((children, &[])),
        Some(i) => {
            if i + 1 != children.len() {
                return Err(err(
                    span,
                    "<if>: <else> must be the last child (nothing after </else>)",
                ));
            }
            let Node::Element { attrs, children: else_kids, span: espan, .. } = &children[i] else {
                unreachable!()
            };
            if !attrs.is_empty() {
                return Err(err(*espan, "<else> takes no attributes"));
            }
            Ok((&children[..i], else_kids))
        }
    }
}

fn element(
    node: &Node,
    scope: &Scope,
    registry: &ComponentRegistry,
    slot_children: Option<&[Node]>,
) -> Result<ViewNode, InterpError> {
    let Node::Element { name, attrs, children, span } = node else {
        return Err(err(
            Span { line: 0, col: 0 },
            "text content is only allowed inside <Text>, <Button>, and <Badge>",
        ));
    };

    if name == "slot" {
        if !attrs.is_empty() {
            return Err(err(*span, "<slot/> takes no attributes"));
        }
        let Some(slot_children) = slot_children else {
            return Err(err(*span, "<slot/> is only valid in a component with a slot prop"));
        };
        let nodes = resolve_children(slot_children, scope, registry, None, "slot", *span)?;
        if nodes.len() == 1 {
            return Ok(nodes.into_iter().next().unwrap());
        }
        return Ok(ViewNode::Column { style: BoxStyle::default(), children: nodes });
    }

    if !is_catalog_element(name) {
        return Err(err(
            *span,
            format!("unknown component <{name}> — no component file defines it (§7.2)"),
        ));
    }

    match name.as_str() {
        "Column" | "Row" => {
            let mut style = BoxStyle::default();
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("gap", AttrValue::Num(v)) => style.gap = *v,
                    ("padding", AttrValue::Num(v)) => style.padding = *v,
                    ("grow", AttrValue::Flag) => style.grow = 1.0,
                    // Weighted grow (`grow={2.5}`): proportional pane
                    // shares that stay responsive under window resize.
                    ("grow", AttrValue::Num(v)) => style.grow = *v,
                    ("width", AttrValue::Num(v)) => style.width = Some(*v),
                    ("cross", AttrValue::Str(s)) if s == "center" => style.cross_center = true,
                    ("main", AttrValue::Str(s)) if s == "center" => style.main_center = true,
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let kids = resolve_children(children, scope, registry, slot_children, name, *span)?;
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
            let mut wrap = false;
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
                    ("wrap", AttrValue::Flag) => wrap = true,
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let content = text_content(children, scope, *span)?;
            Ok(ViewNode::Text { content, size, bold, muted, grow, wrap, test_id })
        }

        "Button" => {
            let mut variant = Variant::Secondary;
            // (handler name, optional arg binding, span)
            let mut on_press: Option<(String, Option<String>, Span)> = None;
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
                    ("onPress", AttrValue::Ident(m)) => on_press = Some((m.clone(), None, a.span)),
                    ("onPress", AttrValue::Call { name: m, arg }) => {
                        on_press = Some((m.clone(), Some(arg.clone()), a.span))
                    }
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let (msg, arg, mspan) =
                on_press.ok_or_else(|| err(*span, "<Button> requires onPress={someAction}"))?;
            let on_press_arg = match &arg {
                Some(binding) => {
                    require_camel_case(&msg, mspan)?;
                    if !(scope.has_str_action)(&msg) {
                        return Err(err(
                            mspan,
                            format!(
                                "action '{msg}' takes no payload — `onPress={{{msg}({binding})}}` \
                                 needs a String-payload action (fn {msg}(&mut self, s: String))"
                            ),
                        ));
                    }
                    let value = (scope.field)(binding).ok_or_else(|| {
                        err(mspan, format!("no binding '{binding}' in scope for onPress={{{msg}({binding})}}"))
                    })?;
                    Some(value)
                }
                None => None,
            };
            let on_press = match &arg {
                Some(_) => {
                    require_camel_case(&msg, mspan)?;
                    if !(scope.has_str_action)(&msg) {
                        return Err(err(
                            mspan,
                            format!("'{msg}' is not a String-payload Action variant"),
                        ));
                    }
                    msg.clone()
                }
                None => resolve_unit_handler(scope, &msg, mspan)?,
            };
            let label = text_content(children, scope, *span)?;
            Ok(ViewNode::Button { label, variant, on_press, on_press_arg, test_id })
        }

        "Badge" => {
            let mut tone = Tone::Neutral;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("tone", AttrValue::Str(s)) => {
                        tone = match s.as_str() {
                            "neutral" => Tone::Neutral,
                            "danger" => Tone::Danger,
                            "warning" => Tone::Warning,
                            "success" => Tone::Success,
                            "info" => Tone::Info,
                            other => {
                                return Err(err(
                                    a.span,
                                    format!("unknown tone \"{other}\" — expected \"neutral\", \"danger\", \"warning\", \"success\" or \"info\""),
                                ))
                            }
                        }
                    }
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let label = text_content(children, scope, *span)?;
            Ok(ViewNode::Badge { label, tone, test_id })
        }

        // §7.1 (PLAN-CATALOG wave 5): pressable pill. `selected` follows
        // the bool Display-"true" rule; `onPress` / `onRemove` are
        // optional and accept the item-scoped Call form.
        "Chip" => {
            let mut selected = false;
            let mut on_press: Option<(String, Option<String>, Span)> = None;
            let mut on_remove: Option<(String, Option<String>, Span)> = None;
            let mut test_id = None;
            let resolve = |path: &str, aspan: Span| -> Result<String, InterpError> {
                (scope.field)(path)
                    .ok_or_else(|| err(aspan, format!("no binding '{path}' in scope")))
            };
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("selected", AttrValue::Flag) => selected = true,
                    ("selected", AttrValue::Ident(p)) => selected = resolve(p, a.span)? == "true",
                    ("onPress", AttrValue::Ident(m)) => on_press = Some((m.clone(), None, a.span)),
                    ("onPress", AttrValue::Call { name: m, arg }) => {
                        on_press = Some((m.clone(), Some(arg.clone()), a.span))
                    }
                    ("onRemove", AttrValue::Ident(m)) => on_remove = Some((m.clone(), None, a.span)),
                    ("onRemove", AttrValue::Call { name: m, arg }) => {
                        on_remove = Some((m.clone(), Some(arg.clone()), a.span))
                    }
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let label = text_content(children, scope, *span)?;
            let resolve_handler = |spec: Option<(String, Option<String>, Span)>, attr: &str| {
                match spec {
                    None => Ok((None, None)),
                    Some((m, arg, mspan)) => {
                        require_camel_case(&m, mspan)?;
                        let resolved_arg = match &arg {
                            Some(binding) => {
                                if !(scope.has_str_action)(&m) {
                                    return Err(err(
                                        mspan,
                                        format!(
                                            "action '{m}' takes no payload — `{attr}={{{m}({binding})}}` \
                                             needs a String-payload action (fn {m}(&mut self, s: String))"
                                        ),
                                    ));
                                }
                                Some(resolve(binding, mspan)?)
                            }
                            None => {
                                if !(scope.has_action)(&m) {
                                    return Err(err(
                                        mspan,
                                        format!("no action '{m}' on Action — check the enum"),
                                    ));
                                }
                                None
                            }
                        };
                        Ok((Some(m), resolved_arg))
                    }
                }
            };
            let (on_press, on_press_arg) = resolve_handler(on_press, "onPress")?;
            let (on_remove, on_remove_arg) = resolve_handler(on_remove, "onRemove")?;
            Ok(ViewNode::Chip {
                label,
                selected,
                on_press,
                on_press_arg,
                on_remove,
                on_remove_arg,
                test_id,
            })
        }

        // §7.1 (PLAN-CATALOG wave 7): tab strip. Children resolve
        // through resolve_children so `<for>` / `<if>` work — Gloss
        // binds `tab_rows` this way.
        "Tabs" => {
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let kids = resolve_children(children, scope, registry, slot_children, name, *span)?;
            Ok(ViewNode::Tabs { children: kids, test_id })
        }

        // §7.1 (PLAN-CATALOG wave 7): chrome strip. `vertical` is the
        // Gloss left rail; default is a full-width row.
        "Toolbar" => {
            let mut vertical = false;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("vertical", AttrValue::Flag) => vertical = true,
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let kids = resolve_children(children, scope, registry, slot_children, name, *span)?;
            Ok(ViewNode::Toolbar { vertical, children: kids, test_id })
        }

        // §7.1 (PLAN-CATALOG wave 7): footer chrome. Children are
        // ordinary markup (Gloss muted Text slots).
        "StatusBar" => {
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let kids = resolve_children(children, scope, registry, slot_children, name, *span)?;
            Ok(ViewNode::StatusBar { children: kids, test_id })
        }

        // One tab. Child text is the label; selected / onPress /
        // onClose mirror Chip (onClose is the editor-chrome name).
        "Tab" => {
            let mut selected = false;
            let mut on_press: Option<(String, Option<String>, Span)> = None;
            let mut on_close: Option<(String, Option<String>, Span)> = None;
            let mut test_id = None;
            let resolve = |path: &str, aspan: Span| -> Result<String, InterpError> {
                (scope.field)(path)
                    .ok_or_else(|| err(aspan, format!("no binding '{path}' in scope")))
            };
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("selected", AttrValue::Flag) => selected = true,
                    ("selected", AttrValue::Ident(p)) => selected = resolve(p, a.span)? == "true",
                    ("onPress", AttrValue::Ident(m)) => on_press = Some((m.clone(), None, a.span)),
                    ("onPress", AttrValue::Call { name: m, arg }) => {
                        on_press = Some((m.clone(), Some(arg.clone()), a.span))
                    }
                    ("onClose", AttrValue::Ident(m)) => on_close = Some((m.clone(), None, a.span)),
                    ("onClose", AttrValue::Call { name: m, arg }) => {
                        on_close = Some((m.clone(), Some(arg.clone()), a.span))
                    }
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let label = text_content(children, scope, *span)?;
            let resolve_handler = |spec: Option<(String, Option<String>, Span)>, attr: &str| {
                match spec {
                    None => Ok((None, None)),
                    Some((m, arg, mspan)) => {
                        require_camel_case(&m, mspan)?;
                        let resolved_arg = match &arg {
                            Some(binding) => {
                                if !(scope.has_str_action)(&m) {
                                    return Err(err(
                                        mspan,
                                        format!(
                                            "action '{m}' takes no payload — `{attr}={{{m}({binding})}}` \
                                             needs a String-payload action (fn {m}(&mut self, s: String))"
                                        ),
                                    ));
                                }
                                Some(resolve(binding, mspan)?)
                            }
                            None => {
                                if !(scope.has_action)(&m) {
                                    return Err(err(
                                        mspan,
                                        format!("no action '{m}' on Action — check the enum"),
                                    ));
                                }
                                None
                            }
                        };
                        Ok((Some(m), resolved_arg))
                    }
                }
            };
            let (on_press, on_press_arg) = resolve_handler(on_press, "onPress")?;
            let (on_close, on_close_arg) = resolve_handler(on_close, "onClose")?;
            Ok(ViewNode::Tab {
                label,
                selected,
                on_press,
                on_press_arg,
                on_close,
                on_close_arg,
                test_id,
            })
        }

        "Heading" => {
            let mut level = 1u8;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("level", AttrValue::Num(v)) => level = (*v as u8).clamp(1, 4),
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let content = text_content(children, scope, *span)?;
            Ok(ViewNode::Heading { content, level, test_id })
        }

        "Progress" => {
            let mut value = 0.0f32;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    // Literal or a bound numeric State field. The binding is
                    // typed: interp parses the field's Display (f32 round-trips)
                    // and release codegen accesses it directly, so rustc rejects
                    // a non-numeric field at the `.lx` line (§6.5).
                    ("value", AttrValue::Num(v)) => value = *v,
                    ("value", AttrValue::Ident(path)) => {
                        let s = (scope.field)(path)
                            .ok_or_else(|| err(a.span, format!("no field '{path}' on State")))?;
                        value = s.parse::<f32>().map_err(|_| {
                            err(a.span, format!("field '{path}' is not a number — <Progress value> expects 0..1"))
                        })?;
                    }
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            if !children.is_empty() {
                return Err(err(*span, "<Progress> takes no children"));
            }
            Ok(ViewNode::Progress { value, test_id })
        }


        // §7.1 (PLAN-CATALOG): inline status callout — tone like Badge,
        // title literal-or-binding, body from text content.
        "Alert" => {
            let mut tone = Tone::Neutral;
            let mut title: Option<String> = None;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("tone", AttrValue::Str(t)) => tone = parse_tone(t, a.span)?,
                    ("title", AttrValue::Str(v)) => title = Some(v.clone()),
                    ("title", AttrValue::Ident(p)) => {
                        title = Some((scope.field)(p).ok_or_else(|| {
                            err(a.span, format!("no binding '{p}' in scope"))
                        })?)
                    }
                    ("testId", AttrValue::Str(v)) => test_id = Some(v.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let title =
                title.ok_or_else(|| err(*span, "<Alert> requires title={binding} or title=\"…\""))?;
            let body = text_content(children, scope, *span)?;
            Ok(ViewNode::Alert { tone, title, body, test_id })
        }

        // §7.1 (PLAN-CATALOG wave 7): detached non-modal overlay.
        // Mount via `<if>`; optional onDismiss; timeout is an app Timer.
        "Toast" => {
            let mut tone = Tone::Neutral;
            let mut title = String::new();
            let mut on_dismiss: Option<(String, Span)> = None;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("tone", AttrValue::Str(t)) => tone = parse_tone(t, a.span)?,
                    ("title", AttrValue::Str(v)) => title = v.clone(),
                    ("title", AttrValue::Ident(p)) => {
                        title = (scope.field)(p).ok_or_else(|| {
                            err(a.span, format!("no binding '{p}' in scope"))
                        })?;
                    }
                    ("onDismiss", AttrValue::Ident(m)) => on_dismiss = Some((m.clone(), a.span)),
                    ("testId", AttrValue::Str(v)) => test_id = Some(v.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let body = text_content(children, scope, *span)?;
            if title.is_empty() && body.is_empty() {
                return Err(err(*span, "<Toast> needs title=… or body text"));
            }
            let on_dismiss = match on_dismiss {
                None => None,
                Some((m, mspan)) => {
                    require_camel_case(&m, mspan)?;
                    if !(scope.has_action)(&m) {
                        return Err(err(mspan, format!("no action '{m}' on Action — check the enum")));
                    }
                    Some(m)
                }
            };
            Ok(ViewNode::Toast { tone, title, body, on_dismiss, test_id })
        }

        // §7.1 (PLAN-CATALOG): surface containers — Column semantics
        // with token surface defaults.
        "Card" | "Panel" => {
            let mut style = BoxStyle::default();
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("gap", AttrValue::Num(v)) => style.gap = *v,
                    ("padding", AttrValue::Num(v)) => style.padding = *v,
                    ("grow", AttrValue::Flag) => style.grow = 1.0,
                    ("grow", AttrValue::Num(v)) => style.grow = *v,
                    ("width", AttrValue::Num(v)) => style.width = Some(*v),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let kids = resolve_children(children, scope, registry, slot_children, name, *span)?;
            Ok(if name == "Card" {
                ViewNode::Card { style, children: kids }
            } else {
                ViewNode::Panel { style, children: kids }
            })
        }

        // §7.1 (PLAN-CATALOG): static loading placeholder.
        "Skeleton" => {
            let mut width = None;
            let mut height = 14.0;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("width", AttrValue::Num(v)) => width = Some(*v),
                    ("height", AttrValue::Num(v)) => height = *v,
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            if !children.is_empty() {
                return Err(err(*span, "<Skeleton> takes no children"));
            }
            Ok(ViewNode::Skeleton { width, height })
        }

        // §7.1 (PLAN-CATALOG): initials avatar.
        "Avatar" => {
            let mut avatar_name: Option<String> = None;
            let mut size = 32.0;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("name", AttrValue::Str(v)) => avatar_name = Some(v.clone()),
                    ("name", AttrValue::Ident(p)) => {
                        avatar_name = Some((scope.field)(p).ok_or_else(|| {
                            err(a.span, format!("no binding '{p}' in scope"))
                        })?)
                    }
                    ("size", AttrValue::Num(v)) => size = *v,
                    ("testId", AttrValue::Str(v)) => test_id = Some(v.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let avatar_name = avatar_name
                .ok_or_else(|| err(*span, "<Avatar> requires name={binding} or name=\"…\""))?;
            if !children.is_empty() {
                return Err(err(*span, "<Avatar> takes no children"));
            }
            Ok(ViewNode::Avatar { name: avatar_name, size, test_id })
        }

        // §7.1 (PLAN-CATALOG): model-owned on/off. `checked` follows
        // the bool Display-"true" rule; `onToggle` is a unit action.
        "Switch" => {
            let mut checked = false;
            let mut on_toggle: Option<(String, Span)> = None;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("checked", AttrValue::Flag) => checked = true,
                    ("checked", AttrValue::Ident(p)) => {
                        checked = (scope.field)(p)
                            .ok_or_else(|| err(a.span, format!("no binding '{p}' in scope")))?
                            == "true"
                    }
                    ("onToggle", AttrValue::Ident(m)) => on_toggle = Some((m.clone(), a.span)),
                    ("testId", AttrValue::Str(v)) => test_id = Some(v.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let (msg, mspan) =
                on_toggle.ok_or_else(|| err(*span, "<Switch> requires onToggle={someAction}"))?;
            require_camel_case(&msg, mspan)?;
            if !(scope.has_action)(&msg) {
                return Err(err(mspan, format!("no action '{msg}' on Action — check the enum")));
            }
            if !children.is_empty() {
                return Err(err(*span, "<Switch> takes no children"));
            }
            Ok(ViewNode::Switch { checked, on_toggle: msg, test_id })
        }

        // §7.1 (PLAN-CATALOG): model-owned binary choice; label is the
        // text content.
        "Checkbox" => {
            let mut checked = false;
            let mut on_toggle: Option<(String, Span)> = None;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("checked", AttrValue::Flag) => checked = true,
                    ("checked", AttrValue::Ident(p)) => {
                        checked = (scope.field)(p)
                            .ok_or_else(|| err(a.span, format!("no binding '{p}' in scope")))?
                            == "true"
                    }
                    ("onToggle", AttrValue::Ident(m)) => on_toggle = Some((m.clone(), a.span)),
                    ("testId", AttrValue::Str(v)) => test_id = Some(v.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let (msg, mspan) =
                on_toggle.ok_or_else(|| err(*span, "<Checkbox> requires onToggle={someAction}"))?;
            require_camel_case(&msg, mspan)?;
            if !(scope.has_action)(&msg) {
                return Err(err(mspan, format!("no action '{msg}' on Action — check the enum")));
            }
            let label = text_content(children, scope, *span)?;
            Ok(ViewNode::Checkbox { checked, on_toggle: msg, label, test_id })
        }

        // §7.1 (PLAN-CATALOG): single choice; item-scoped
        // `onSelect={pick(opt.id)}` builds groups from a `<for>`.
        "Radio" => {
            let mut selected = false;
            let mut on_select: Option<(String, Option<String>, Span)> = None;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("selected", AttrValue::Flag) => selected = true,
                    ("selected", AttrValue::Ident(p)) => {
                        selected = (scope.field)(p)
                            .ok_or_else(|| err(a.span, format!("no binding '{p}' in scope")))?
                            == "true"
                    }
                    ("onSelect", AttrValue::Ident(m)) => on_select = Some((m.clone(), None, a.span)),
                    ("onSelect", AttrValue::Call { name: m, arg }) => {
                        on_select = Some((m.clone(), Some(arg.clone()), a.span))
                    }
                    ("testId", AttrValue::Str(v)) => test_id = Some(v.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let (msg, arg, mspan) =
                on_select.ok_or_else(|| err(*span, "<Radio> requires onSelect={someAction}"))?;
            require_camel_case(&msg, mspan)?;
            let on_select_arg = match &arg {
                Some(binding) => {
                    if !(scope.has_str_action)(&msg) {
                        return Err(err(
                            mspan,
                            format!("action '{msg}' takes no payload — `onSelect={{{msg}({binding})}}` needs a String-payload action"),
                        ));
                    }
                    Some((scope.field)(binding).ok_or_else(|| {
                        err(mspan, format!("no binding '{binding}' in scope"))
                    })?)
                }
                None => {
                    if !(scope.has_action)(&msg) {
                        return Err(err(mspan, format!("no action '{msg}' on Action — check the enum")));
                    }
                    None
                }
            };
            let label = text_content(children, scope, *span)?;
            Ok(ViewNode::Radio { selected, on_select: msg, on_select_arg, label, test_id })
        }

        // §7.1 (PLAN-CATALOG): edge-anchored modal (bottom = Sheet).
        "Drawer" => {
            let mut side = DrawerSide::Left;
            let mut on_dismiss: Option<String> = None;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("side", AttrValue::Str(v)) => {
                        side = match v.as_str() {
                            "left" => DrawerSide::Left,
                            "right" => DrawerSide::Right,
                            "bottom" => DrawerSide::Bottom,
                            other => {
                                return Err(err(
                                    a.span,
                                    format!("unknown side \"{other}\" — expected \"left\", \"right\" or \"bottom\""),
                                ))
                            }
                        }
                    }
                    ("onDismiss", AttrValue::Ident(m)) => {
                        require_camel_case(m, a.span)?;
                        if !(scope.has_action)(m) {
                            return Err(err(a.span, format!("no action '{m}' on Action — check the enum")));
                        }
                        on_dismiss = Some(m.clone());
                    }
                    ("testId", AttrValue::Str(v)) => test_id = Some(v.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let kids = resolve_children(children, scope, registry, slot_children, name, *span)?;
            Ok(ViewNode::Drawer { side, on_dismiss, test_id, children: kids })
        }


        // §7.1 (PLAN-CATALOG): hierarchy trail — chevron auto-
        // inserted by build(); children are BreadcrumbItem.
        "Breadcrumb" => {
            let kids = resolve_children(children, scope, registry, slot_children, name, *span)?;
            Ok(ViewNode::Breadcrumb { children: kids })
        }

        // §7.1: one crumb. `current` follows the same bool rule as
        // `<if>`; non-current crumbs may take an onPress (incl. the
        // item-scoped `action(loopVar.field)` form for `<for>` trails).
        "BreadcrumbItem" => {
            let mut label: Option<String> = None;
            let mut current = false;
            let mut on_press: Option<(String, Option<String>, Span)> = None;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("label", AttrValue::Str(v)) => label = Some(v.clone()),
                    ("label", AttrValue::Ident(p)) => {
                        label = Some((scope.field)(p).ok_or_else(|| {
                            err(a.span, format!("no binding '{p}' in scope"))
                        })?)
                    }
                    ("current", AttrValue::Flag) => current = true,
                    ("current", AttrValue::Ident(p)) => {
                        current = (scope.field)(p)
                            .ok_or_else(|| err(a.span, format!("no binding '{p}' in scope")))?
                            == "true"
                    }
                    ("onPress", AttrValue::Ident(m)) => on_press = Some((m.clone(), None, a.span)),
                    ("onPress", AttrValue::Call { name: m, arg }) => {
                        on_press = Some((m.clone(), Some(arg.clone()), a.span))
                    }
                    ("testId", AttrValue::Str(v)) => test_id = Some(v.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let label = label
                .ok_or_else(|| err(*span, "<BreadcrumbItem> requires label={binding} or label=\"…\""))?;
            let (on_press, on_press_arg) = match on_press {
                None => (None, None),
                Some((m, arg, mspan)) => {
                    require_camel_case(&m, mspan)?;
                    let resolved_arg = match &arg {
                        Some(binding) => {
                            if !(scope.has_str_action)(&m) {
                                return Err(err(
                                    mspan,
                                    format!("action '{m}' takes no payload — `onPress={{{m}({binding})}}` needs a String-payload action"),
                                ));
                            }
                            Some((scope.field)(binding).ok_or_else(|| {
                                err(mspan, format!("no binding '{binding}' in scope"))
                            })?)
                        }
                        None => {
                            if !(scope.has_action)(&m) {
                                return Err(err(mspan, format!("no action '{m}' on Action — check the enum")));
                            }
                            None
                        }
                    };
                    (Some(m), resolved_arg)
                }
            };
            if !children.is_empty() {
                return Err(err(*span, "<BreadcrumbItem> takes no children"));
            }
            Ok(ViewNode::BreadcrumbItem { label, current, on_press, on_press_arg, test_id })
        }

        // §7.1: fully generated page-nav row — the windowing logic
        // lives once in libra-components, shared by both engines.
        "Pagination" => {
            let mut current: Option<u32> = None;
            let mut total: Option<u32> = None;
            let mut on_page: Option<(String, Span)> = None;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("current", AttrValue::Num(v)) => current = Some(*v as u32),
                    ("current", AttrValue::Ident(p)) => {
                        let s = (scope.field)(p).ok_or_else(|| err(a.span, format!("no field '{p}' on State")))?;
                        current = Some(s.parse::<u32>().map_err(|_| {
                            err(a.span, format!("field '{p}' is not a number — <Pagination current> expects a page index"))
                        })?);
                    }
                    ("total", AttrValue::Num(v)) => total = Some(*v as u32),
                    ("total", AttrValue::Ident(p)) => {
                        let s = (scope.field)(p).ok_or_else(|| err(a.span, format!("no field '{p}' on State")))?;
                        total = Some(s.parse::<u32>().map_err(|_| {
                            err(a.span, format!("field '{p}' is not a number — <Pagination total> expects a page count"))
                        })?);
                    }
                    ("onPage", AttrValue::Ident(m)) => on_page = Some((m.clone(), a.span)),
                    ("testId", AttrValue::Str(v)) => test_id = Some(v.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let current = current.ok_or_else(|| err(*span, "<Pagination> requires current={0-based page}"))?;
            let total = total.ok_or_else(|| err(*span, "<Pagination> requires total={page count}"))?;
            let (msg, mspan) =
                on_page.ok_or_else(|| err(*span, "<Pagination> requires onPage={someAction}"))?;
            require_camel_case(&msg, mspan)?;
            if !(scope.has_str_action)(&msg) {
                return Err(err(
                    mspan,
                    format!("'{msg}' is not a String-payload Action variant (need `{msg}(String)`)"),
                ));
            }
            if !children.is_empty() {
                return Err(err(*span, "<Pagination> takes no children"));
            }
            Ok(ViewNode::Pagination { current, total, on_page: msg, test_id })
        }

        // §7.1: progress stages — status is derived by the container
        // from index vs `current`, never an attr on the item.
        "Stepper" => {
            let mut current: Option<u32> = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("current", AttrValue::Num(v)) => current = Some(*v as u32),
                    ("current", AttrValue::Ident(p)) => {
                        let s = (scope.field)(p).ok_or_else(|| err(a.span, format!("no field '{p}' on State")))?;
                        current = Some(s.parse::<u32>().map_err(|_| {
                            err(a.span, format!("field '{p}' is not a number — <Stepper current> expects a step index"))
                        })?);
                    }
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let current = current.ok_or_else(|| err(*span, "<Stepper> requires current={0-based step index}"))?;
            let kids = resolve_children(children, scope, registry, slot_children, name, *span)?;
            Ok(ViewNode::Stepper { current, children: kids })
        }

        "StepperItem" => {
            let mut label: Option<String> = None;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("label", AttrValue::Str(v)) => label = Some(v.clone()),
                    ("label", AttrValue::Ident(p)) => {
                        label = Some((scope.field)(p).ok_or_else(|| {
                            err(a.span, format!("no binding '{p}' in scope"))
                        })?)
                    }
                    ("testId", AttrValue::Str(v)) => test_id = Some(v.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let label =
                label.ok_or_else(|| err(*span, "<StepperItem> requires label={binding} or label=\"…\""))?;
            if !children.is_empty() {
                return Err(err(*span, "<StepperItem> takes no children"));
            }
            Ok(ViewNode::StepperItem { label, test_id })
        }

        // §7.1: ledger list — `done` is a direct attr per entry
        // (unlike Stepper, not position-derived); the container draws
        // the connector.
        "Timeline" => {
            let kids = resolve_children(children, scope, registry, slot_children, name, *span)?;
            Ok(ViewNode::Timeline { children: kids })
        }

        "TimelineItem" => {
            let mut title: Option<String> = None;
            let mut meta = String::new();
            let mut done = false;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("title", AttrValue::Str(v)) => title = Some(v.clone()),
                    ("title", AttrValue::Ident(p)) => {
                        title = Some((scope.field)(p).ok_or_else(|| {
                            err(a.span, format!("no binding '{p}' in scope"))
                        })?)
                    }
                    ("meta", AttrValue::Str(v)) => meta = v.clone(),
                    ("meta", AttrValue::Ident(p)) => {
                        meta = (scope.field)(p)
                            .ok_or_else(|| err(a.span, format!("no binding '{p}' in scope")))?
                    }
                    ("done", AttrValue::Flag) => done = true,
                    ("done", AttrValue::Ident(p)) => {
                        done = (scope.field)(p)
                            .ok_or_else(|| err(a.span, format!("no binding '{p}' in scope")))?
                            == "true"
                    }
                    ("testId", AttrValue::Str(v)) => test_id = Some(v.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let title =
                title.ok_or_else(|| err(*span, "<TimelineItem> requires title={binding} or title=\"…\""))?;
            if !children.is_empty() {
                return Err(err(*span, "<TimelineItem> takes no children"));
            }
            Ok(ViewNode::TimelineItem { title, meta, done, test_id })
        }

        // §7.1 (PLAN-CATALOG): model-owned continuous control — typed
        // value (the Progress rule), String-payload onChange.
        "Slider" => {
            let mut value = 0.0f32;
            let mut on_change: Option<(String, Span)> = None;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("value", AttrValue::Num(v)) => value = *v,
                    ("value", AttrValue::Ident(path)) => {
                        let sv = (scope.field)(path)
                            .ok_or_else(|| err(a.span, format!("no field '{path}' on State")))?;
                        value = sv.parse::<f32>().map_err(|_| {
                            err(a.span, format!("field '{path}' is not a number — <Slider value> expects 0..1"))
                        })?;
                    }
                    ("onChange", AttrValue::Ident(m)) => on_change = Some((m.clone(), a.span)),
                    ("testId", AttrValue::Str(v)) => test_id = Some(v.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let (msg, mspan) =
                on_change.ok_or_else(|| err(*span, "<Slider> requires onChange={someAction}"))?;
            require_camel_case(&msg, mspan)?;
            if !(scope.has_str_action)(&msg) {
                return Err(err(
                    mspan,
                    format!("'{msg}' is not a String-payload Action variant (need `{msg}(String)`)"),
                ));
            }
            let test_id = test_id
                .ok_or_else(|| err(*span, "<Slider> requires testId=\"…\" (its drag identity)"))?;
            if !children.is_empty() {
                return Err(err(*span, "<Slider> takes no children"));
            }
            Ok(ViewNode::Slider { value, on_change: msg, test_id })
        }

        // §7.1 (PLAN-CATALOG): hover-input tooltip wrapper.
        "Tooltip" => {
            let mut text: Option<String> = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("text", AttrValue::Str(v)) => text = Some(v.clone()),
                    ("text", AttrValue::Ident(p)) => {
                        text = Some((scope.field)(p).ok_or_else(|| {
                            err(a.span, format!("no binding '{p}' in scope"))
                        })?)
                    }
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let text =
                text.ok_or_else(|| err(*span, "<Tooltip> requires text={binding} or text=\"…\""))?;
            let kids = resolve_children(children, scope, registry, slot_children, name, *span)?;
            Ok(ViewNode::Tooltip { text, children: kids })
        }

        // §7.1 (PLAN-CATALOG / PLAN-NOTES-APP M7): the anchored menu —
        // the Dialog overlay machinery, positioned by the menu-anchor
        // input. Children are ordinary markup (ghost Buttons as items).
        "ContextMenu" => {
            let mut on_dismiss: Option<String> = None;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("onDismiss", AttrValue::Ident(m)) => {
                        require_camel_case(m, a.span)?;
                        if !(scope.has_action)(m) {
                            return Err(err(
                                a.span,
                                format!("no action '{m}' on Action — check the enum"),
                            ));
                        }
                        on_dismiss = Some(m.clone());
                    }
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let test_id = test_id
                .ok_or_else(|| err(*span, "<ContextMenu> requires testId=\"…\" (its anchor identity)"))?;
            let kids = resolve_children(children, scope, registry, slot_children, name, *span)?;
            Ok(ViewNode::ContextMenu { on_dismiss, test_id, children: kids })
        }

        // §7.1 (PLAN-CATALOG / PLAN-NOTES-APP M5): the two-pane
        // splitter. `fraction` is typed like `<Progress value>`;
        // `onResize` is a String-payload action (the new fraction's
        // Display); `testId` is the drag identity.
        "Split" => {
            let mut fraction = 0.5f32;
            let mut on_resize: Option<(String, Span)> = None;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("fraction", AttrValue::Num(v)) => fraction = *v,
                    ("fraction", AttrValue::Ident(path)) => {
                        let s = (scope.field)(path)
                            .ok_or_else(|| err(a.span, format!("no field '{path}' on State")))?;
                        fraction = s.parse::<f32>().map_err(|_| {
                            err(a.span, format!("field '{path}' is not a number — <Split fraction> expects 0..1"))
                        })?;
                    }
                    ("onResize", AttrValue::Ident(m)) => on_resize = Some((m.clone(), a.span)),
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let (msg, mspan) =
                on_resize.ok_or_else(|| err(*span, "<Split> requires onResize={someAction}"))?;
            require_camel_case(&msg, mspan)?;
            if !(scope.has_str_action)(&msg) {
                return Err(err(
                    mspan,
                    format!("'{msg}' is not a String-payload Action variant (need `{msg}(String)`)"),
                ));
            }
            let test_id = test_id
                .ok_or_else(|| err(*span, "<Split> requires testId=\"…\" (its drag identity)"))?;
            let kids = resolve_children(children, scope, registry, slot_children, name, *span)?;
            if kids.len() != 2 {
                return Err(err(
                    *span,
                    format!("<Split> takes exactly two children (panes), got {}", kids.len()),
                ));
            }
            Ok(ViewNode::Split { fraction, on_resize: msg, test_id, children: kids })
        }

        // §7.1 (PLAN-CATALOG / PLAN-NOTES-APP M6): the in-tree modal.
        // Mount/unmount is the app's `<if>`; `title` is a literal or
        // binding, `onDismiss` an optional unit action for
        // click-outside dismissal.
        "Dialog" => {
            let mut title: Option<String> = None;
            let mut on_dismiss: Option<String> = None;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("title", AttrValue::Str(s)) => title = Some(s.clone()),
                    ("title", AttrValue::Ident(p)) => {
                        title = Some((scope.field)(p).ok_or_else(|| {
                            err(a.span, format!("no binding '{p}' in scope"))
                        })?)
                    }
                    ("onDismiss", AttrValue::Ident(m)) => {
                        require_camel_case(m, a.span)?;
                        if !(scope.has_action)(m) {
                            return Err(err(
                                a.span,
                                format!("no action '{m}' on Action — check the enum"),
                            ));
                        }
                        on_dismiss = Some(m.clone());
                    }
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let title = title
                .ok_or_else(|| err(*span, "<Dialog> requires title={binding} or title=\"…\""))?;
            let kids = resolve_children(children, scope, registry, slot_children, name, *span)?;
            Ok(ViewNode::Dialog { title, on_dismiss, test_id, children: kids })
        }

        "Scroll" => {
            let mut height = None;
            let mut test_id = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("height", AttrValue::Num(v)) => height = Some(*v),
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let test_id = test_id
                .ok_or_else(|| err(*span, "<Scroll> requires testId=\"…\" (its scroll-position key)"))?;
            let kids = resolve_children(children, scope, registry, slot_children, name, *span)?;
            Ok(ViewNode::Scroll { height, test_id, children: kids })
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
            require_camel_case(&msg, mspan)?;
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

        // §7.1 (PLAN-CATALOG wave 3): Input chrome + optional Enter
        // submit. `onChange` is a String-payload action; `onSubmit` is
        // an optional unit action. `testId` is the focus identity.
        "Search" => {
            let mut value: Option<(String, Span)> = None;
            let mut placeholder = String::new();
            let mut on_change: Option<(String, Span)> = None;
            let mut on_submit: Option<String> = None;
            let mut test_id: Option<String> = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("value", AttrValue::Ident(path)) => value = Some((path.clone(), a.span)),
                    ("placeholder", AttrValue::Str(s)) => placeholder = s.clone(),
                    ("onChange", AttrValue::Ident(m)) => on_change = Some((m.clone(), a.span)),
                    ("onSubmit", AttrValue::Ident(m)) => {
                        require_camel_case(m, a.span)?;
                        if !(scope.has_action)(m) {
                            return Err(err(
                                a.span,
                                format!("no action '{m}' on Action — check the enum"),
                            ));
                        }
                        on_submit = Some(m.clone());
                    }
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let (path, vspan) =
                value.ok_or_else(|| err(*span, "<Search> requires value={someField}"))?;
            let value = (scope.field)(&path)
                .ok_or_else(|| err(vspan, format!("no field '{path}' on State")))?;
            let (msg, mspan) =
                on_change.ok_or_else(|| err(*span, "<Search> requires onChange={someAction}"))?;
            require_camel_case(&msg, mspan)?;
            if !(scope.has_str_action)(&msg) {
                return Err(err(
                    mspan,
                    format!("'{msg}' is not a String-payload Action variant (need `{msg}(String)`)"),
                ));
            }
            let test_id = test_id
                .ok_or_else(|| err(*span, "<Search> requires testId=\"…\" (focus identity)"))?;
            Ok(ViewNode::Search {
                value,
                placeholder,
                on_change: msg,
                on_submit,
                test_id,
            })
        }

        // §7.1 (PLAN-CATALOG): a pressable list row — title/subtitle/
        // meta lines + a `selected` wash. Attrs are literals or
        // bindings (loop fields included); `selected` follows the
        // `<if>` bool rule: resolved Display equals "true".
        "ListItem" => {
            let mut title: Option<String> = None;
            let mut subtitle = String::new();
            let mut meta = String::new();
            let mut selected = false;
            let mut on_press: Option<(String, Option<String>, Span)> = None;
            let mut on_menu: Option<(String, Option<String>, Span)> = None;
            let mut test_id = None;
            let resolve = |path: &str, aspan: Span| -> Result<String, InterpError> {
                (scope.field)(path)
                    .ok_or_else(|| err(aspan, format!("no binding '{path}' in scope")))
            };
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("title", AttrValue::Str(s)) => title = Some(s.clone()),
                    ("title", AttrValue::Ident(p)) => title = Some(resolve(p, a.span)?),
                    ("subtitle", AttrValue::Str(s)) => subtitle = s.clone(),
                    ("subtitle", AttrValue::Ident(p)) => subtitle = resolve(p, a.span)?,
                    ("meta", AttrValue::Str(s)) => meta = s.clone(),
                    ("meta", AttrValue::Ident(p)) => meta = resolve(p, a.span)?,
                    ("selected", AttrValue::Flag) => selected = true,
                    ("selected", AttrValue::Ident(p)) => selected = resolve(p, a.span)? == "true",
                    ("onPress", AttrValue::Ident(m)) => on_press = Some((m.clone(), None, a.span)),
                    ("onPress", AttrValue::Call { name: m, arg }) => {
                        on_press = Some((m.clone(), Some(arg.clone()), a.span))
                    }
                    ("onMenu", AttrValue::Ident(m)) => on_menu = Some((m.clone(), None, a.span)),
                    ("onMenu", AttrValue::Call { name: m, arg }) => {
                        on_menu = Some((m.clone(), Some(arg.clone()), a.span))
                    }
                    ("testId", AttrValue::Str(s)) => test_id = Some(s.clone()),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            if !children.is_empty() {
                return Err(err(*span, "<ListItem> takes no children — use title/subtitle/meta attrs"));
            }
            let title =
                title.ok_or_else(|| err(*span, "<ListItem> requires title={binding} or title=\"…\""))?;
            let (msg, arg, mspan) =
                on_press.ok_or_else(|| err(*span, "<ListItem> requires onPress={someAction}"))?;
            require_camel_case(&msg, mspan)?;
            let on_press_arg = match &arg {
                Some(binding) => {
                    if !(scope.has_str_action)(&msg) {
                        return Err(err(
                            mspan,
                            format!(
                                "action '{msg}' takes no payload — `onPress={{{msg}({binding})}}` \
                                 needs a String-payload action (fn {msg}(&mut self, s: String))"
                            ),
                        ));
                    }
                    Some(resolve(binding, mspan)?)
                }
                None => {
                    if !(scope.has_action)(&msg) {
                        return Err(err(mspan, format!("no action '{msg}' on Action — check the enum")));
                    }
                    None
                }
            };
            // M7: right-click handler, same dispatchability rules as onPress.
            let (on_menu, on_menu_arg) = match on_menu {
                None => (None, None),
                Some((m, arg, mspan)) => {
                    require_camel_case(&m, mspan)?;
                    let resolved_arg = match &arg {
                        Some(binding) => {
                            if !(scope.has_str_action)(&m) {
                                return Err(err(
                                    mspan,
                                    format!(
                                        "action '{m}' takes no payload — `onMenu={{{m}({binding})}}` \
                                         needs a String-payload action (fn {m}(&mut self, s: String))"
                                    ),
                                ));
                            }
                            Some(resolve(binding, mspan)?)
                        }
                        None => {
                            if !(scope.has_action)(&m) {
                                return Err(err(mspan, format!("no action '{m}' on Action — check the enum")));
                            }
                            None
                        }
                    };
                    (Some(m), resolved_arg)
                }
            };
            Ok(ViewNode::ListItem {
                title,
                subtitle,
                meta,
                selected,
                on_press: msg,
                on_press_arg,
                on_menu,
                on_menu_arg,
                test_id,
            })
        }

        // §10.1 stage 1 (PLAN-TEXTAREA): multi-line controlled entry —
        // the Input contract verbatim, one node kind over.
        "Textarea" => {
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
                value.ok_or_else(|| err(*span, "<Textarea> requires value={someField}"))?;
            let value = (scope.field)(&path)
                .ok_or_else(|| err(vspan, format!("no field '{path}' on State")))?;
            let (msg, mspan) =
                on_change.ok_or_else(|| err(*span, "<Textarea> requires onChange={someAction}"))?;
            require_camel_case(&msg, mspan)?;
            if !(scope.has_str_action)(&msg) {
                return Err(err(
                    mspan,
                    format!("'{msg}' is not a String-payload Action variant (need `{msg}(String)`)"),
                ));
            }
            let test_id = test_id
                .ok_or_else(|| err(*span, "<Textarea> requires testId=\"…\" (focus identity)"))?;
            Ok(ViewNode::Textarea { value, placeholder, on_change: msg, test_id })
        }

        // §7.1 Layout (PLAN-CATALOG wave 1): pure layout — fixed room
        // or a flex spring. No children, no handlers.
        "Spacer" => {
            let mut size: Option<f32> = None;
            let mut grow = false;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("size", AttrValue::Num(v)) => size = Some(*v),
                    ("grow", AttrValue::Flag) => grow = true,
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            if size.is_some() && grow {
                return Err(err(*span, "<Spacer> takes size={px} or grow, not both"));
            }
            if !children.is_empty() {
                return Err(err(*span, "<Spacer> takes no children"));
            }
            // Bare <Spacer/> is a spring, same muscle memory as SwiftUI.
            Ok(ViewNode::Spacer { size })
        }

        // §7.1 Layout (PLAN-CATALOG wave 1): a 1px hairline rule —
        // horizontal by default (Column), `vertical` for Row seams.
        "Separator" => {
            let mut vertical = false;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("vertical", AttrValue::Flag) => vertical = true,
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            if !children.is_empty() {
                return Err(err(*span, "<Separator> takes no children"));
            }
            Ok(ViewNode::Separator { vertical })
        }

        // §12.5: invisible, zero-layout — declares a mount-scoped
        // subscription rather than rendering anything.
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
            require_camel_case(&msg, mspan)?;
            if !(scope.has_frame_info_action)(&msg) {
                return Err(err(
                    mspan,
                    format!("'{msg}' is not a FrameInfo-payload Action variant (need `{msg}(FrameInfo)`)"),
                ));
            }
            Ok(ViewNode::SourceFrames { on_tick: msg })
        }

        // §12.5: invisible, zero-layout — a recurring timer while
        // mounted, identity `(every_ms, on_tick)`.
        "Interval" => {
            let mut every: Option<(u64, Span)> = None;
            let mut on_tick: Option<(String, Span)> = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("every", AttrValue::Num(v)) => every = Some((*v as u64, a.span)),
                    ("onTick", AttrValue::Ident(m)) => on_tick = Some((m.clone(), a.span)),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let (every_ms, _espan) =
                every.ok_or_else(|| err(*span, "<Interval> requires every={milliseconds}"))?;
            let (msg, mspan) =
                on_tick.ok_or_else(|| err(*span, "<Interval> requires onTick={someAction}"))?;
            require_camel_case(&msg, mspan)?;
            if !(scope.has_instant_action)(&msg) {
                return Err(err(
                    mspan,
                    format!("'{msg}' is not an Instant-payload Action variant (need `{msg}(Instant)`)"),
                ));
            }
            Ok(ViewNode::SourceInterval { every_ms, on_tick: msg })
        }

        // §12.5: grammar + mount + query in v1 — `id` resolves like any
        // other binding; full event delivery (Ws/Spawn) is a later
        // change (PLAN-SOURCES).
        "Stream" => {
            let mut id: Option<(String, Span)> = None;
            let mut on_event: Option<(String, Span)> = None;
            for a in attrs {
                match (a.name.as_str(), &a.value) {
                    ("id", AttrValue::Ident(path)) => id = Some((path.clone(), a.span)),
                    ("onEvent", AttrValue::Ident(m)) => on_event = Some((m.clone(), a.span)),
                    _ => return Err(unknown_attr(name, a)),
                }
            }
            let (path, ispan) = id.ok_or_else(|| err(*span, "<Stream> requires id={someField}"))?;
            let id_value = (scope.field)(&path)
                .ok_or_else(|| err(ispan, format!("no field '{path}' on State")))?;
            let (msg, mspan) =
                on_event.ok_or_else(|| err(*span, "<Stream> requires onEvent={someAction}"))?;
            require_camel_case(&msg, mspan)?;
            if !(scope.has_str_action)(&msg) {
                return Err(err(
                    mspan,
                    format!("'{msg}' is not a String-payload Action variant (need `{msg}(String)`)"),
                ));
            }
            Ok(ViewNode::SourceStream { id: id_value, on_event: msg })
        }

        "for" => Err(err(*span, "<for> is only allowed as a direct child of <Column>/<Row>")),
        "if" => Err(err(*span, "<if> is only allowed as a direct child of <Column>/<Row>")),
        "else" => Err(err(*span, "<else> is only allowed as a direct child of <if>")),

        other => Err(err(
            *span,
            format!("unknown element <{other}> — available: Column, Row, Card, Panel, Scroll, Split, Dialog, Drawer, ContextMenu, Alert, Skeleton, Avatar, Switch, Checkbox, Radio, Slider, Tooltip, Breadcrumb, BreadcrumbItem, Pagination, Stepper, StepperItem, Timeline, TimelineItem, Text, Button, Badge, Chip, Tabs, Tab, Toast, Toolbar, StatusBar, Heading, Progress, Chart, Sparkline, Input, Search, Textarea, ListItem, Spacer, Separator, Frames, Interval, Stream, for, if, slot, or a registered component (§7.2)"),
        )),
    }
}

/// Badge/Alert `tone="…"` table (§7.1 status tones).
fn parse_tone(t: &str, span: Span) -> Result<Tone, InterpError> {
    Ok(match t {
        "neutral" => Tone::Neutral,
        "danger" => Tone::Danger,
        "warning" => Tone::Warning,
        "success" => Tone::Success,
        "info" => Tone::Info,
        other => {
            return Err(err(
                span,
                format!("unknown tone \"{other}\" — expected neutral/danger/warning/success/info"),
            ))
        }
    })
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
                    let v = (scope.field)(path).ok_or_else(|| {
                        if scope.binding_ctx == "props" {
                            err(*span, format!("no prop '{path}' — components resolve bindings only against props (§7.2)"))
                        } else {
                            err(*span, format!("no field '{path}' on State"))
                        }
                    })?;
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
            collection_rows: &|_| None,
            has_frame_info_action: &|_| false,
            has_instant_action: &|_| false,
            binding_ctx: "State",
        }
    }

    fn empty_registry() -> ComponentRegistry {
        ComponentRegistry::new()
    }

    #[test]
    fn resolves_counter_view() {
        let ast = parse(
            r#"<Column gap={12}>
                 <Text testId="count">{count}</Text>
                 <Button variant="primary" onPress={increment}>+</Button>
               </Column>"#,
        )
        .unwrap();
        let field = |p: &str| (p == "count").then(|| "7".to_string());
        let has_action = |m: &str| m == "increment";
        let view = resolve(&ast, &scope_ok(&field, &has_action), &empty_registry()).unwrap();
        let ViewNode::Column { children, .. } = view else { panic!() };
        assert_eq!(
            children[0],
            ViewNode::Text {
                content: "7".into(),
                size: TextSize::Md,
                bold: false,
                muted: false,
                grow: false,
                wrap: false,
                test_id: Some("count".into())
            }
        );
    }

    #[test]
    fn typoed_handler_is_a_load_time_error_with_span() {
        let ast = parse("<Column>\n  <Button onPress={incremnt}>+</Button>\n</Column>").unwrap();
        let field = |_: &str| None;
        let has_action = |m: &str| m == "increment";
        let e = resolve(&ast, &scope_ok(&field, &has_action), &empty_registry()).unwrap_err();
        assert!(e.message.contains("no action 'incremnt' on Action"));
        assert_eq!(e.span.line, 2);
    }

    #[test]
    fn pascalcase_handler_name_is_a_camel_case_style_error() {
        // §3.3: `onPress={Reset}` (the pre-revision style) misreads as
        // a component reference in JSX-flavored markup — rejected
        // before any ReflectAction lookup, with a camelCase suggestion.
        let ast = parse("<Column>\n  <Button onPress={Reset}>Reset</Button>\n</Column>").unwrap();
        let field = |_: &str| None;
        let has_action = |m: &str| m == "reset" || m == "Reset";
        let e = resolve(&ast, &scope_ok(&field, &has_action), &empty_registry()).unwrap_err();
        assert!(e.message.contains("must be camelCase"), "{}", e.message);
        assert!(e.message.contains("did you mean 'reset'"), "{}", e.message);
    }

    #[test]
    fn typoed_binding_is_a_load_time_error() {
        let ast = parse("<Column><Text>{cuont}</Text></Column>").unwrap();
        let field = |p: &str| (p == "count").then(|| "0".into());
        let has_action = |_: &str| true;
        let e = resolve(&ast, &scope_ok(&field, &has_action), &empty_registry()).unwrap_err();
        assert!(e.message.contains("no field 'cuont' on State"));
    }

    // Scope where `toggle` is a String-payload action and `item` is a
    // bound value — the shape a `<for>` body sees for an item-scoped
    // handler (§6.2).
    fn scope_item<'a>(
        field: &'a dyn Fn(&str) -> Option<String>,
        has_str_action: &'a dyn Fn(&str) -> bool,
    ) -> Scope<'a> {
        Scope {
            field,
            has_action: &|_| false,
            has_str_action,
            collection_rows: &|_| None,
            has_frame_info_action: &|_| false,
            has_instant_action: &|_| false,
            binding_ctx: "State",
        }
    }

    #[test]
    fn item_scoped_handler_binds_the_loop_value_as_payload() {
        let ast = parse(r#"<Column><Button onPress={toggle(item)}>x</Button></Column>"#).unwrap();
        let field = |p: &str| (p == "item").then(|| "todo-42".to_string());
        let has_str = |m: &str| m == "toggle";
        let view = resolve(&ast, &scope_item(&field, &has_str), &empty_registry()).unwrap();
        let ViewNode::Column { children, .. } = view else { panic!() };
        let ViewNode::Button { on_press, on_press_arg, .. } = &children[0] else { panic!() };
        assert_eq!(on_press, "toggle");
        assert_eq!(on_press_arg.as_deref(), Some("todo-42"));
    }

    #[test]
    fn item_scoped_handler_on_a_unit_action_is_an_error() {
        // `toggle` takes no String payload → `toggle(item)` is rejected.
        let ast = parse(r#"<Column><Button onPress={toggle(item)}>x</Button></Column>"#).unwrap();
        let field = |p: &str| (p == "item").then(|| "todo-42".to_string());
        let has_str = |_: &str| false;
        let e = resolve(&ast, &scope_item(&field, &has_str), &empty_registry()).unwrap_err();
        assert!(e.message.contains("takes no payload"), "{}", e.message);
    }

    #[test]
    fn item_scoped_handler_with_unbound_arg_is_an_error() {
        let ast = parse(r#"<Column><Button onPress={toggle(gone)}>x</Button></Column>"#).unwrap();
        let field = |_: &str| None;
        let has_str = |m: &str| m == "toggle";
        let e = resolve(&ast, &scope_item(&field, &has_str), &empty_registry()).unwrap_err();
        assert!(e.message.contains("no binding 'gone'"), "{}", e.message);
    }

    #[test]
    fn unknown_element_is_an_error() {
        let ast = parse("<Colunm></Colunm>").unwrap();
        let field = |_: &str| None;
        let has_action = |_: &str| true;
        let e = resolve(&ast, &scope_ok(&field, &has_action), &empty_registry()).unwrap_err();
        assert!(e.message.contains("unknown component <Colunm>"));
    }

    fn scope_with_collection<'a>(
        field: &'a dyn Fn(&str) -> Option<String>,
        collection_rows: &'a dyn Fn(&str) -> Option<Vec<CollectionRow>>,
    ) -> Scope<'a> {
        Scope {
            field,
            has_action: &|_| true,
            has_str_action: &|_| false,
            collection_rows,
            has_frame_info_action: &|_| false,
            has_instant_action: &|_| false,
            binding_ctx: "State",
        }
    }

    fn opaque_items(items: Vec<&str>) -> Vec<CollectionRow> {
        items.into_iter().map(|s| CollectionRow::opaque(s.to_string())).collect()
    }

    #[test]
    fn for_loop_expands_into_one_node_per_item_bound_to_the_loop_variable() {
        let ast = parse(
            r#"<Column><for each={items} as="x" key={x}><Text>{x}</Text></for></Column>"#,
        )
        .unwrap();
        let field = |_: &str| None;
        let items = opaque_items(vec!["a", "b", "c"]);
        let collection_rows = move |p: &str| (p == "items").then(|| items.clone());
        let view = resolve(&ast, &scope_with_collection(&field, &collection_rows), &empty_registry()).unwrap();
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
        let items = opaque_items(vec!["a"]);
        let collection_rows = move |p: &str| (p == "items").then(|| items.clone());
        let view = resolve(&ast, &scope_with_collection(&field, &collection_rows), &empty_registry()).unwrap();
        let ViewNode::Column { children, .. } = view else { panic!() };
        let ViewNode::Text { content, .. } = &children[0] else { panic!() };
        assert_eq!(content, "a - 7");
    }

    #[test]
    fn for_requires_each() {
        let ast = parse(r#"<Column><for as="x" key={x}><Text>{x}</Text></for></Column>"#).unwrap();
        let field = |_: &str| None;
        let collection_rows = |_: &str| None;
        let e = resolve(&ast, &scope_with_collection(&field, &collection_rows), &empty_registry()).unwrap_err();
        assert!(e.message.contains("requires each="));
    }

    #[test]
    fn for_requires_key() {
        let ast = parse(r#"<Column><for each={items} as="x"><Text>{x}</Text></for></Column>"#).unwrap();
        let field = |_: &str| None;
        let items = opaque_items(vec!["a"]);
        let collection_rows = move |p: &str| (p == "items").then(|| items.clone());
        let e = resolve(&ast, &scope_with_collection(&field, &collection_rows), &empty_registry()).unwrap_err();
        assert!(e.message.contains("requires key="));
    }

    #[test]
    fn for_key_may_be_a_single_field_on_the_loop_variable() {
        let ast = parse(
            r#"<Column><for each={items} as="x" key={x.id}><Text>{x.label}</Text></for></Column>"#,
        )
        .unwrap();
        let field = |_: &str| None;
        let items = vec![CollectionRow {
            whole: "row-1".into(),
            fields: vec![("id".into(), "1".into()), ("label".into(), "Alpha".into())],
        }];
        let collection_rows = move |p: &str| (p == "items").then(|| items.clone());
        let view = resolve(&ast, &scope_with_collection(&field, &collection_rows), &empty_registry()).unwrap();
        let ViewNode::Column { children, .. } = view else { panic!() };
        let ViewNode::Text { content, .. } = &children[0] else { panic!() };
        assert_eq!(content, "Alpha");
    }

    #[test]
    fn for_key_rejects_nested_field_paths() {
        let ast = parse(
            r#"<Column><for each={items} as="x" key={x.id.extra}><Text>{x}</Text></for></Column>"#,
        )
        .unwrap();
        let field = |_: &str| None;
        let items = opaque_items(vec!["a"]);
        let collection_rows = move |p: &str| (p == "items").then(|| items.clone());
        let e = resolve(&ast, &scope_with_collection(&field, &collection_rows), &empty_registry()).unwrap_err();
        assert!(e.message.contains("one field segment"), "{}", e.message);
    }

    #[test]
    fn for_key_must_name_the_loop_variable() {
        let ast = parse(
            r#"<Column><for each={items} as="x" key={other}><Text>{x}</Text></for></Column>"#,
        )
        .unwrap();
        let field = |_: &str| None;
        let items = opaque_items(vec!["a"]);
        let collection_rows = move |p: &str| (p == "items").then(|| items.clone());
        let e = resolve(&ast, &scope_with_collection(&field, &collection_rows), &empty_registry()).unwrap_err();
        assert!(e.message.contains("key must be {x}"), "{}", e.message);
    }

    #[test]
    fn item_scoped_handler_can_bind_a_field_payload() {
        let ast = parse(
            r#"<Column><for each={items} as="t" key={t.id}><Button onPress={complete(t.id)}>{t.text}</Button></for></Column>"#,
        )
        .unwrap();
        let field = |_: &str| None;
        let items = vec![CollectionRow {
            whole: "1".into(),
            fields: vec![("id".into(), "1".into()), ("text".into(), "Buy milk".into())],
        }];
        let collection_rows = move |p: &str| (p == "items").then(|| items.clone());
        let mut scope = scope_with_collection(&field, &collection_rows);
        let has_str = |m: &str| m == "complete";
        scope.has_str_action = &has_str;
        let view = resolve(&ast, &scope, &empty_registry()).unwrap();
        let ViewNode::Column { children, .. } = view else { panic!() };
        let ViewNode::Button { on_press, on_press_arg, label, .. } = &children[0] else { panic!() };
        assert_eq!(on_press, "complete");
        assert_eq!(on_press_arg.as_deref(), Some("1"));
        assert_eq!(label, "Buy milk");
    }

    #[test]
    fn for_names_a_real_collection_or_it_is_a_load_time_error() {
        let ast = parse(
            r#"<Column><for each={nonexistent} as="x" key={x}><Text>{x}</Text></for></Column>"#,
        )
        .unwrap();
        let field = |_: &str| None;
        let collection_rows = |_: &str| None;
        let e = resolve(&ast, &scope_with_collection(&field, &collection_rows), &empty_registry()).unwrap_err();
        assert!(e.message.contains("no collection 'nonexistent' on State"));
    }

    fn scope_with_frame_info<'a>(has_frame_info_action: &'a dyn Fn(&str) -> bool) -> Scope<'a> {
        Scope {
            field: &|_| None,
            has_action: &|_| true,
            has_str_action: &|_| false,
            collection_rows: &|_| None,
            has_frame_info_action,
            has_instant_action: &|_| false,
            binding_ctx: "State",
        }
    }

    fn scope_with_instant<'a>(has_instant_action: &'a dyn Fn(&str) -> bool) -> Scope<'a> {
        Scope {
            field: &|_| None,
            has_action: &|_| true,
            has_str_action: &|_| false,
            collection_rows: &|_| None,
            has_frame_info_action: &|_| false,
            has_instant_action,
            binding_ctx: "State",
        }
    }

    #[test]
    fn frames_resolves_to_a_source_node_when_on_tick_names_a_real_handler() {
        let ast = parse(r#"<Column><Frames onTick={frame} /></Column>"#).unwrap();
        let has_frame_info_action = |m: &str| m == "frame";
        let view = resolve(&ast, &scope_with_frame_info(&has_frame_info_action), &empty_registry()).unwrap();
        let ViewNode::Column { children, .. } = view else { panic!() };
        assert_eq!(children, vec![ViewNode::SourceFrames { on_tick: "frame".into() }]);
    }

    #[test]
    fn frames_requires_on_tick() {
        let ast = parse(r#"<Column><Frames /></Column>"#).unwrap();
        let has_frame_info_action = |_: &str| true;
        let e = resolve(&ast, &scope_with_frame_info(&has_frame_info_action), &empty_registry()).unwrap_err();
        assert!(e.message.contains("requires onTick="));
    }

    #[test]
    fn frames_on_tick_must_name_a_frame_info_handler() {
        let ast = parse(r#"<Column><Frames onTick={notAHandler} /></Column>"#).unwrap();
        let has_frame_info_action = |_: &str| false;
        let e = resolve(&ast, &scope_with_frame_info(&has_frame_info_action), &empty_registry()).unwrap_err();
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
        let items = opaque_items(vec!["a", "b"]);
        let collection_rows = move |p: &str| (p == "items").then(|| items.clone());
        let has_frame_info_action = |m: &str| m == "frame";
        let scope = Scope {
            field: &field,
            has_action: &|_| true,
            has_str_action: &|_| false,
            collection_rows: &collection_rows,
            has_frame_info_action: &has_frame_info_action,
            has_instant_action: &|_| false,
            binding_ctx: "State",
        };
        let view = resolve(&ast, &scope, &empty_registry()).unwrap();
        let ViewNode::Column { children, .. } = view else { panic!() };
        assert_eq!(children.len(), 2);
        for child in &children {
            assert_eq!(child, &ViewNode::SourceFrames { on_tick: "frame".into() });
        }
    }

    #[test]
    fn if_takes_the_then_branch_when_test_displays_true() {
        let ast = parse(
            r#"<Column><if test={running}><Text>on</Text><else><Text>off</Text></else></if></Column>"#,
        )
        .unwrap();
        let field = |p: &str| (p == "running").then(|| "true".to_string());
        let view = resolve(&ast, &scope_ok(&field, &|_| true), &empty_registry()).unwrap();
        let ViewNode::Column { children, .. } = view else { panic!() };
        assert_eq!(children.len(), 1);
        let ViewNode::Text { content, .. } = &children[0] else { panic!() };
        assert_eq!(content, "on");
    }

    #[test]
    fn if_takes_the_else_branch_when_test_is_not_true() {
        let ast = parse(
            r#"<Column><if test={running}><Text>on</Text><else><Text>off</Text></else></if></Column>"#,
        )
        .unwrap();
        let field = |p: &str| (p == "running").then(|| "false".to_string());
        let view = resolve(&ast, &scope_ok(&field, &|_| true), &empty_registry()).unwrap();
        let ViewNode::Column { children, .. } = view else { panic!() };
        let ViewNode::Text { content, .. } = &children[0] else { panic!() };
        assert_eq!(content, "off");
    }

    #[test]
    fn if_without_else_emits_nothing_when_false() {
        let ast = parse(r#"<Column><if test={running}><Text>on</Text></if></Column>"#).unwrap();
        let field = |p: &str| (p == "running").then(|| "false".to_string());
        let view = resolve(&ast, &scope_ok(&field, &|_| true), &empty_registry()).unwrap();
        let ViewNode::Column { children, .. } = view else { panic!() };
        assert!(children.is_empty());
    }

    #[test]
    fn if_requires_test() {
        let ast = parse(r#"<Column><if><Text>x</Text></if></Column>"#).unwrap();
        let e = resolve(&ast, &scope_ok(&|_| None, &|_| true), &empty_registry()).unwrap_err();
        assert!(e.message.contains("requires test="));
    }

    #[test]
    fn interval_resolves_to_a_source_node_when_on_tick_names_a_real_handler() {
        let ast = parse(r#"<Column><Interval every={1000} onTick={poll} /></Column>"#).unwrap();
        let has_instant_action = |m: &str| m == "poll";
        let view = resolve(&ast, &scope_with_instant(&has_instant_action), &empty_registry()).unwrap();
        let ViewNode::Column { children, .. } = view else { panic!() };
        assert_eq!(
            children,
            vec![ViewNode::SourceInterval { every_ms: 1000, on_tick: "poll".into() }]
        );
    }

    #[test]
    fn interval_requires_every_and_on_tick() {
        let has_instant_action = |_: &str| true;
        let missing_every = parse(r#"<Column><Interval onTick={poll} /></Column>"#).unwrap();
        let e = resolve(&missing_every, &scope_with_instant(&has_instant_action), &empty_registry()).unwrap_err();
        assert!(e.message.contains("requires every="), "{}", e.message);

        let missing_on_tick = parse(r#"<Column><Interval every={1000} /></Column>"#).unwrap();
        let e = resolve(&missing_on_tick, &scope_with_instant(&has_instant_action), &empty_registry()).unwrap_err();
        assert!(e.message.contains("requires onTick="), "{}", e.message);
    }

    #[test]
    fn interval_on_tick_must_name_an_instant_handler() {
        let ast = parse(r#"<Column><Interval every={1000} onTick={notAHandler} /></Column>"#).unwrap();
        let has_instant_action = |_: &str| false;
        let e = resolve(&ast, &scope_with_instant(&has_instant_action), &empty_registry()).unwrap_err();
        assert!(e.message.contains("not an Instant-payload Action variant"), "{}", e.message);
    }

    #[test]
    fn stream_resolves_to_a_source_node_with_the_resolved_id_and_a_str_handler() {
        let ast = parse(r#"<Column><Stream id={feed} onEvent={packet} /></Column>"#).unwrap();
        let field = |p: &str| (p == "feed").then(|| "telemetry".to_string());
        let has_str = |m: &str| m == "packet";
        let view = resolve(&ast, &scope_item(&field, &has_str), &empty_registry()).unwrap();
        let ViewNode::Column { children, .. } = view else { panic!() };
        assert_eq!(
            children,
            vec![ViewNode::SourceStream { id: "telemetry".into(), on_event: "packet".into() }]
        );
    }

    #[test]
    fn stream_requires_id_and_on_event() {
        let has_str = |_: &str| true;
        let missing_id = parse(r#"<Column><Stream onEvent={packet} /></Column>"#).unwrap();
        let e = resolve(&missing_id, &scope_item(&|_| None, &has_str), &empty_registry()).unwrap_err();
        assert!(e.message.contains("requires id="), "{}", e.message);

        let missing_on_event = parse(r#"<Column><Stream id={feed} /></Column>"#).unwrap();
        let field = |p: &str| (p == "feed").then(|| "telemetry".to_string());
        let e = resolve(&missing_on_event, &scope_item(&field, &has_str), &empty_registry()).unwrap_err();
        assert!(e.message.contains("requires onEvent="), "{}", e.message);
    }

    #[test]
    fn stream_on_event_must_name_a_str_handler() {
        let ast = parse(r#"<Column><Stream id={feed} onEvent={notAHandler} /></Column>"#).unwrap();
        let field = |p: &str| (p == "feed").then(|| "telemetry".to_string());
        let has_str = |_: &str| false;
        let e = resolve(&ast, &scope_item(&field, &has_str), &empty_registry()).unwrap_err();
        assert!(e.message.contains("not a String-payload Action variant"), "{}", e.message);
    }

    #[test]
    fn stream_id_must_resolve_to_a_real_binding() {
        let ast = parse(r#"<Column><Stream id={gone} onEvent={packet} /></Column>"#).unwrap();
        let has_str = |m: &str| m == "packet";
        let e = resolve(&ast, &scope_item(&|_| None, &has_str), &empty_registry()).unwrap_err();
        assert!(e.message.contains("no field 'gone' on State"), "{}", e.message);
    }
}
