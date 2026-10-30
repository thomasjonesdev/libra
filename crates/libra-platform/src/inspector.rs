//! Dev inspector (§18.4). A second window beside the app shows the
//! action log and the current state. It is not part of the app's
//! display list, so it stays off the determinism contract.
//!
//! `libra dev` sets `LIBRA_INSPECTOR=1`. Debug builds open the window.
//! Damage rects, the layer cache, and a frame-timing view are still open.

use std::collections::VecDeque;
use std::fmt::Debug;

use libra_components::{layout, BoxStyle, TextMeasurer, TextSize, ViewNode};
use libra_draw::Theme;

pub(crate) const CAP: usize = 48;

/// Ring of recent actions. Identical neighbors collapse (`Ticked × 12`)
/// so a `<Frames>` pump does not erase the clicks around it.
#[derive(Clone, Debug)]
pub(crate) struct ActionLog {
    lines: VecDeque<String>,
    cap: usize,
}

impl ActionLog {
    pub(crate) fn new(cap: usize) -> Self {
        ActionLog {
            lines: VecDeque::new(),
            cap: cap.max(1),
        }
    }

    pub(crate) fn push(&mut self, event: &str) {
        if let Some(last) = self.lines.back_mut() {
            let (base, n) = split_repeat(last);
            if base == event {
                let shown = n.saturating_add(1);
                *last = if shown == 1 {
                    event.to_string()
                } else {
                    format!("{event} × {shown}")
                };
                return;
            }
        }
        self.lines.push_back(event.to_string());
        while self.lines.len() > self.cap {
            self.lines.pop_front();
        }
    }

    pub(crate) fn lines(&self) -> Vec<String> {
        self.lines.iter().cloned().collect()
    }
}

impl Default for ActionLog {
    fn default() -> Self {
        ActionLog::new(CAP)
    }
}

fn split_repeat(line: &str) -> (&str, u32) {
    let Some((base, n)) = line.rsplit_once(" × ") else {
        return (line, 1);
    };
    match n.parse() {
        Ok(n) => (base, n),
        Err(_) => (line, 1),
    }
}

/// One line for the log. Long `Debug` payloads are cut so a single
/// action cannot fill the window.
pub(crate) fn brief_debug(msg: &impl Debug) -> String {
    let full = format!("{msg:?}");
    let mut chars = full.chars();
    let brief: String = chars.by_ref().take(237).collect();
    if chars.next().is_some() {
        format!("{brief}...")
    } else {
        brief
    }
}

pub(crate) fn enabled() -> bool {
    cfg!(debug_assertions) && std::env::var("LIBRA_INSPECTOR").ok().as_deref() == Some("1")
}

fn text(content: &str, bold: bool, test_id: &str) -> ViewNode {
    ViewNode::Text {
        content: content.to_string(),
        size: if bold { TextSize::Md } else { TextSize::Sm },
        bold,
        muted: !bold,
        grow: false,
        wrap: true,
        test_id: Some(test_id.to_string()),
    }
}

/// State rows, then the action log. Empty sections say so.
pub(crate) fn view(fields: &[(String, String)], actions: &[String]) -> ViewNode {
    let mut children = vec![text("Inspector", true, "inspector-title"), text("State", true, "state-heading")];
    if fields.is_empty() {
        children.push(text("(no fields)", false, "state-empty"));
    } else {
        for (i, (name, value)) in fields.iter().enumerate() {
            children.push(text(&format!("{name}  {value}"), false, &format!("field-{i}")));
        }
    }
    children.push(text("Actions", true, "actions-heading"));
    if actions.is_empty() {
        children.push(text("(none yet)", false, "log-empty"));
    } else {
        for (i, entry) in actions.iter().enumerate() {
            children.push(text(entry, false, &format!("log-{i}")));
        }
    }
    ViewNode::Column {
        style: BoxStyle {
            gap: 4.0,
            padding: 12.0,
            ..BoxStyle::default()
        },
        children,
    }
}

pub(crate) fn laid_texts(
    fields: &[(String, String)],
    actions: &[String],
    measurer: &dyn TextMeasurer,
) -> Vec<String> {
    let view = view(fields, actions);
    let tree = layout(&view, (320.0, 520.0), measurer, &Theme::dark()).expect("inspector layout");
    tree.items
        .iter()
        .map(|item| item.semantics.text.clone())
        .filter(|text| !text.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use libra_draw::FontStyle;

    struct Fixed;
    impl TextMeasurer for Fixed {
        fn measure(&self, text: &str, px: f32, _: FontStyle) -> (f32, f32) {
            (text.chars().count() as f32 * px * 0.5, px)
        }
    }

    #[test]
    fn repeats_collapse_and_the_ring_drops_the_oldest() {
        let mut log = ActionLog::new(3);
        log.push("Increment");
        log.push("Ticked(0)");
        log.push("Ticked(0)");
        log.push("Ticked(0)");
        assert_eq!(
            log.lines(),
            vec!["Increment".to_string(), "Ticked(0) × 3".to_string()]
        );
        log.push("Reset");
        log.push("Decrement");
        assert_eq!(
            log.lines(),
            vec![
                "Ticked(0) × 3".to_string(),
                "Reset".to_string(),
                "Decrement".to_string()
            ]
        );
    }

    #[test]
    fn the_view_lists_state_and_actions() {
        let texts = laid_texts(
            &[("count".into(), "3".into()), ("token".into(), "(redacted)".into())],
            &["Increment".into()],
            &Fixed,
        );
        assert!(texts.iter().any(|t| t == "count  3"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "token  (redacted)"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "Increment"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "Inspector"), "{texts:?}");
    }

    #[test]
    fn empty_sections_say_so() {
        let texts = laid_texts(&[], &[], &Fixed);
        assert!(texts.iter().any(|t| t == "(no fields)"));
        assert!(texts.iter().any(|t| t == "(none yet)"));
    }

    #[test]
    fn long_debug_is_cut() {
        let long = "x".repeat(400);
        let brief = brief_debug(&long);
        assert!(brief.chars().count() <= 240, "{}", brief.chars().count());
        assert!(brief.ends_with("..."));
    }
}
