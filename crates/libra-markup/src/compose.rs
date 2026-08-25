//! Composable `.lx` components (ARCHITECTURE.md §7.2, PLAN-COMPOSE): typed
//! `<props>` declarations, filename→PascalCase naming, and a component registry.

use crate::{Node, Span};
use std::collections::HashMap;
use std::path::Path;

/// Prop type in a `<props>` declaration (PLAN-COMPOSE v1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropType {
    String,
    Num,
    Bool,
    Action,
    ActionString,
    /// Children slot — call-site children splice at `<slot/>`.
    Slot,
}

/// One typed parameter in `<props name: Type, …/>`.
#[derive(Debug, Clone, PartialEq)]
pub struct PropParam {
    pub name: String,
    pub ty: PropType,
    pub span: Span,
}

/// A component `.lx` file: `<props …/>` followed by one or more root elements.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentFile {
    pub props: Vec<PropParam>,
    pub props_span: Span,
    pub body: Vec<Node>,
}

/// Parsed view or component file.
#[derive(Debug, Clone, PartialEq)]
pub enum LxFile {
    View(Node),
    Component(ComponentFile),
}

/// A registered component ready for expansion (interp/codegen).
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentDef {
    pub name: String,
    pub props: Vec<PropParam>,
    pub props_span: Span,
    pub body: Vec<Node>,
    /// Source path for diagnostics (optional in tests).
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct ComponentRegistry {
    components: HashMap<String, ComponentDef>,
}

#[derive(Debug, Clone)]
pub struct RegistryError {
    pub message: String,
    pub span: Option<Span>,
    pub source: Option<String>,
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(src) = &self.source {
            if let Some(span) = self.span {
                write!(f, "{src}:{span}: {}", self.message)
            } else {
                write!(f, "{src}: {}", self.message)
            }
        } else {
            write!(f, "{}", self.message)
        }
    }
}
impl std::error::Error for RegistryError {}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self { components: HashMap::new() }
    }

    pub fn get(&self, name: &str) -> Option<&ComponentDef> {
        self.components.get(name)
    }

    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &ComponentDef)> {
        self.components.iter()
    }

    /// Register one component; errors on duplicate names.
    pub fn register(&mut self, def: ComponentDef) -> Result<(), RegistryError> {
        if self.components.contains_key(&def.name) {
            return Err(RegistryError {
                message: format!("duplicate component '{}'", def.name),
                span: Some(def.props_span),
                source: Some(def.source.clone()),
            });
        }
        self.components.insert(def.name.clone(), def);
        Ok(())
    }

    /// Load every component `.lx` under `dir` (recursive). Skips non-component files.
    pub fn load_from_dir(dir: &Path) -> Result<Self, RegistryError> {
        let mut reg = Self::new();
        load_dir_recursive(dir, &mut reg)?;
        reg.detect_cycles()?;
        Ok(reg)
    }

    /// Load components from `(source_path, source_text)` pairs (tests/conformance).
    pub fn load_from_sources(sources: &[(String, String)]) -> Result<Self, RegistryError> {
        let mut reg = Self::new();
        for (path, src) in sources {
            if let LxFile::Component(cf) = crate::parse_file(src).map_err(|e| RegistryError {
                message: e.message,
                span: Some(e.span),
                source: Some(path.clone()),
            })? {
                let stem = Path::new(path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Component");
                let name = filename_to_component_name(stem);
                reg.register(ComponentDef {
                    name,
                    props: cf.props,
                    props_span: cf.props_span,
                    body: cf.body,
                    source: path.clone(),
                })?;
            }
        }
        reg.detect_cycles()?;
        Ok(reg)
    }

    /// DFS cycle detection over component references in bodies.
    pub fn detect_cycles(&self) -> Result<(), RegistryError> {
        for name in self.components.keys() {
            let mut stack = vec![name.clone()];
            let mut visiting = vec![name.clone()];
            if dfs_cycle(self, name, &mut stack, &mut visiting)? {
                let cycle = stack.join(" → ");
                return Err(RegistryError {
                    message: format!("component recursion cycle: {cycle}"),
                    span: None,
                    source: None,
                });
            }
        }
        Ok(())
    }
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn dfs_cycle(
    reg: &ComponentRegistry,
    start: &str,
    stack: &mut Vec<String>,
    visiting: &mut Vec<String>,
) -> Result<bool, RegistryError> {
    let Some(def) = reg.get(start) else { return Ok(false) };
    for node in &def.body {
        for ref_name in component_refs_in_node(node, reg) {
            if visiting.contains(&ref_name) {
                stack.push(ref_name);
                return Ok(true);
            }
            visiting.push(ref_name.clone());
            stack.push(ref_name.clone());
            if dfs_cycle(reg, &ref_name, stack, visiting)? {
                return Ok(true);
            }
            stack.pop();
            visiting.pop();
        }
    }
    Ok(false)
}

fn load_dir_recursive(dir: &Path, reg: &mut ComponentRegistry) -> Result<(), RegistryError> {
    let entries = std::fs::read_dir(dir).map_err(|e| RegistryError {
        message: format!("read dir {}: {e}", dir.display()),
        span: None,
        source: None,
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            load_dir_recursive(&path, reg)?;
        } else if path.extension().is_some_and(|e| e == "lx") {
            let src = std::fs::read_to_string(&path).map_err(|e| RegistryError {
                message: format!("read {}: {e}", path.display()),
                span: None,
                source: Some(path.display().to_string()),
            })?;
            if let LxFile::Component(cf) = crate::parse_file(&src).map_err(|e| RegistryError {
                message: e.message,
                span: Some(e.span),
                source: Some(path.display().to_string()),
            })? {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Component");
                let name = filename_to_component_name(stem);
                reg.register(ComponentDef {
                    name,
                    props: cf.props,
                    props_span: cf.props_span,
                    body: cf.body,
                    source: path.display().to_string(),
                })?;
            }
        }
    }
    Ok(())
}

/// `metric_card.lx` → `MetricCard`.
pub fn filename_to_component_name(stem: &str) -> String {
    stem.split('_')
        .filter(|p| !p.is_empty())
        .map(|part| {
            let mut c = part.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect()
}

/// Catalog + grammar element names — not component references.
pub fn is_catalog_element(name: &str) -> bool {
    matches!(
        name,
        "Column"
            | "Row"
            | "Card"
            | "Panel"
            | "Scroll"
            | "Split"
            | "Dialog"
            | "Drawer"
            | "ContextMenu"
            | "Alert"
            | "Skeleton"
            | "Avatar"
            | "Switch"
            | "Checkbox"
            | "Radio"
            | "Slider"
            | "Tooltip"
            | "Breadcrumb"
            | "BreadcrumbItem"
            | "Pagination"
            | "Stepper"
            | "StepperItem"
            | "Timeline"
            | "TimelineItem"
            | "Text"
            | "Button"
            | "Badge"
            | "Heading"
            | "Progress"
            | "Chart"
            | "Sparkline"
            | "Input"
            | "Search"
            | "Chip"
            | "Tabs"
            | "Tab"
            | "Toast"
            | "Toolbar"
            | "StatusBar"
            | "Textarea"
            | "ListItem"
            | "Markdown"
            | "Spacer"
            | "Separator"
            | "Frames"
            | "Interval"
            | "Stream"
            | "for"
            | "if"
            | "else"
            | "props"
            | "slot"
    )
}

/// Collect PascalCase component references in a node tree.
pub fn component_refs_in_node(node: &Node, reg: &ComponentRegistry) -> Vec<String> {
    let mut out = Vec::new();
    walk_component_refs(node, reg, &mut out);
    out
}

fn walk_component_refs(node: &Node, reg: &ComponentRegistry, out: &mut Vec<String>) {
    match node {
        Node::Text(_) => {}
        Node::Element { name, children, .. } => {
            if !is_catalog_element(name) && reg.get(name).is_some() && !out.contains(name) {
                out.push(name.clone());
            }
            for c in children {
                walk_component_refs(c, reg, out);
            }
        }
    }
}

/// Snake_case helper for codegen fn names: `MetricCard` → `metric_card`.
pub fn component_name_to_snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_to_component_name_snake_case_stem() {
        assert_eq!(filename_to_component_name("metric_card"), "MetricCard");
        assert_eq!(filename_to_component_name("button_demo"), "ButtonDemo");
    }

    #[test]
    fn component_name_to_snake_roundtrip() {
        assert_eq!(component_name_to_snake("MetricCard"), "metric_card");
    }
}
