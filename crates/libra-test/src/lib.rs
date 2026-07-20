//! libra-test — the in-process `Harness<A: App>`: build the real view via the
//! real interpreter, query by semantics, and dispatch through real hit
//! testing and the app's real `update`. Effects are intercepted as data
//! (assert + hand-feed results in any order — no mocks); snapshots compare PNG
//! bytes exactly.
//!
//! Architecture: ARCHITECTURE.md §15 (testing architecture).

// ReflectState/ReflectAction methods resolve through App's supertrait bounds.
use libra_core::{App, Effect, FrameInfo, HttpResult, Instant, KvResult, ReflectAction, SeriesId};
use libra_data::SeriesStore;
use libra_interp::Scope;
use libra_markup::Node;
use libra_raster::Renderer;
use libra_draw::Theme;
use libra_text::{EditOp, EditState};
use libra_components::{InputFocus, LaidTree, Role, ScrollState};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Harness<A: App> {
    app: A,
    ast: Node,
    renderer: Renderer,
    laid: LaidTree,
    viewport: (u32, u32),
    snapshot_dir: PathBuf,
    /// Intercepted, unexecuted effects in emission order (integration
    /// tests, §15.4).
    pending: VecDeque<Effect<A::Msg>>,
    /// The data plane, fed synchronously (tapes) — never by threads.
    store: Arc<SeriesStore>,
    /// §10.1 focus: the focused Input's identity + editing state +
    /// its on_change handler (retained outside the tree until the
    /// reconciler brings structural IDs).
    focus: Option<(String, EditState, String)>,
    /// §11: the theme this harness renders under — part of snapshot
    /// identity (a snapshot's name should say which theme it is).
    theme: Theme,
    /// Hover as an explicit, deterministic paint input (ListItem
    /// washes). `None` (the default) renders the canonical no-hover
    /// frame; `hover(sel)` pins it to an item's center for hover
    /// snapshots.
    hover: Option<(f32, f32)>,
    /// PLAN-SCROLL: scroll positions by `<Scroll>` test_id. A deterministic
    /// input like `hover`/`focus` — empty renders each scroll at the top;
    /// `scroll(id, dy)` moves one.
    scroll_state: ScrollState,
    /// M7 `<ContextMenu>` anchor — pinned by `right_click` to the
    /// clicked item's center; (0,0) otherwise. Deterministic, so
    /// menu-open snapshots are byte-exact.
    menu_anchor: (f32, f32),
}

#[derive(Clone, Debug)]
pub enum Selector {
    TestId(String),
    Text(String),
    Role(Role),
    /// A mounted source element (§12.5), matched by kind —
    /// `"frames"` / `"interval"` / `"stream"`.
    Source(String),
}

pub mod by {
    use super::Selector;
    use libra_components::Role;

    pub fn test_id(id: &str) -> Selector {
        Selector::TestId(id.to_string())
    }
    pub fn text(t: &str) -> Selector {
        Selector::Text(t.to_string())
    }
    pub fn role(r: Role) -> Selector {
        Selector::Role(r)
    }
    pub fn source(kind: &str) -> Selector {
        Selector::Source(kind.to_string())
    }
}

/// Effect matchers for `expect_effect` (§15.4).
pub mod fx {
    #[derive(Clone, Debug)]
    pub enum Matcher {
        Any,
        Http,
        /// Exact key.
        KvGet(Option<String>),
        /// Exact key.
        KvSet(Option<String>),
        KvScan,
        Timer,
        Random,
        SetTheme,
        ClipboardSet,
    }

    pub fn any() -> Matcher {
        Matcher::Any
    }
    pub fn http() -> Matcher {
        Matcher::Http
    }
    pub fn kv_get(key: &str) -> Matcher {
        Matcher::KvGet(Some(key.to_string()))
    }
    pub fn any_kv_get() -> Matcher {
        Matcher::KvGet(None)
    }
    pub fn kv_set(key: &str) -> Matcher {
        Matcher::KvSet(Some(key.to_string()))
    }
    pub fn any_kv_set() -> Matcher {
        Matcher::KvSet(None)
    }
    pub fn kv_scan() -> Matcher {
        Matcher::KvScan
    }
    pub fn timer() -> Matcher {
        Matcher::Timer
    }
    pub fn random() -> Matcher {
        Matcher::Random
    }
    pub fn set_theme() -> Matcher {
        Matcher::SetTheme
    }
    pub fn clipboard_set() -> Matcher {
        Matcher::ClipboardSet
    }
}

fn matches<M>(matcher: &fx::Matcher, c: &Effect<M>) -> bool {
    use fx::Matcher;
    match (matcher, c) {
        (Matcher::Any, _) => true,
        (Matcher::Http, Effect::Http { .. }) => true,
        (Matcher::KvGet(want), Effect::KvGet { key, .. }) => {
            want.as_deref().map_or(true, |w| w == key)
        }
        (Matcher::KvSet(want), Effect::KvSet { key, .. }) => {
            want.as_deref().map_or(true, |w| w == key)
        }
        (Matcher::KvScan, Effect::KvScan { .. }) => true,
        (Matcher::Timer, Effect::Timer { .. }) => true,
        (Matcher::Random, Effect::Random { .. }) => true,
        (Matcher::SetTheme, Effect::SetTheme { .. }) => true,
        (Matcher::ClipboardSet, Effect::ClipboardSet { .. }) => true,
        _ => false,
    }
}

impl<A: App> Harness<A> {
    /// Build from `.lx` source at a default 400×240 viewport.
    pub fn new(lx_source: &str) -> Self {
        Self::with_viewport(lx_source, 400, 240)
    }

    /// Alias for `new` — the documented constructor name (§15.3).
    pub fn render(lx_source: &str) -> Self {
        Self::new(lx_source)
    }

    pub fn with_viewport(lx_source: &str, w: u32, h: u32) -> Self {
        let ast = libra_markup::parse(lx_source)
            .unwrap_or_else(|e| panic!("markup parse error: {e}"));
        let (app, init_effects) = A::init();
        let renderer = Renderer::new();
        let theme = Theme::dark();
        let laid = rebuild(&app, &ast, &renderer, (w, h), &theme);
        Harness {
            app,
            ast,
            renderer,
            laid,
            viewport: (w, h),
            snapshot_dir: PathBuf::from(
                std::env::var("LIBRA_SNAPSHOT_DIR").unwrap_or_else(|_| "tests/snapshots".into()),
            ),
            pending: init_effects.into(),
            store: Arc::new(SeriesStore::new()),
            focus: None,
            theme,
            hover: None,
            scroll_state: ScrollState::new(),
            menu_anchor: (0.0, 0.0),
        }
    }

    /// M7: right-click the selected row — pins the menu anchor to the
    /// item's center and dispatches its `onMenu` handler (item-scoped
    /// arg included), exactly like the shell's right-click.
    pub fn right_click(&mut self, sel: Selector) {
        use libra_components::ItemKind;
        let item = self.find(&sel).unwrap_or_else(|| panic!("right_click: no item matches {sel:?}"));
        let ItemKind::ListItem { on_menu: Some((name, arg)), .. } = &item.kind else {
            panic!("right_click: {sel:?} has no onMenu handler");
        };
        let (name, arg) = (name.clone(), arg.clone());
        self.menu_anchor = item.rect.center();
        let msg = match arg {
            Some(a) => A::Msg::action_with_str(&name, a),
            None => A::Msg::action_by_name(&name),
        }
        .unwrap_or_else(|| panic!("onMenu '{name}' resolved to no Action — reflection out of sync"));
        self.dispatch(msg);
    }

    /// Scroll a `<Scroll testId=…>` viewport by `dy` logical px (down = +).
    /// Clamped at the top by 0 and at the bottom by content overflow — a
    /// deterministic input, so scroll snapshots stay byte-exact (PLAN-SCROLL).
    pub fn scroll(&mut self, test_id: &str, dy: f32) {
        let pos = self.scroll_state.entry(test_id.to_string()).or_insert(0.0);
        *pos = (*pos + dy).max(0.0);
        self.laid = rebuild(&self.app, &self.ast, &self.renderer, self.viewport, &self.theme);
        self.laid.apply_scroll(&self.scroll_state);
        self.laid.apply_menu_anchor(self.menu_anchor);
    }

    /// Pin the hover point to the selected item's center (§7.1
    /// ListItem hover wash) — an explicit input, so hover snapshots
    /// stay byte-exact. `blur_hover()` clears it.
    pub fn hover(&mut self, sel: Selector) {
        let item = self.find(&sel).unwrap_or_else(|| panic!("hover: no item matches {sel:?}"));
        self.hover = Some(item.rect.center());
    }

    pub fn blur_hover(&mut self) {
        self.hover = None;
    }

    /// M5 `<Split>`: set a split's fraction through its `onResize`
    /// handler — the deterministic counterpart of the shell's divider
    /// drag (same clamp, same `{:.4}` payload formatting, so a dragged
    /// and a harness-resized split are byte-identical).
    pub fn resize_split(&mut self, test_id: &str, fraction: f32) {
        use libra_components::ItemKind;
        let on_resize = self
            .laid
            .items
            .iter()
            .find_map(|i| match &i.kind {
                ItemKind::Split { on_resize }
                    if i.semantics.test_id.as_deref() == Some(test_id) =>
                {
                    Some(on_resize.clone())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("resize_split: no <Split testId=\"{test_id}\">"));
        let fraction = fraction.clamp(0.05, 0.95);
        let msg = A::Msg::action_with_str(&on_resize, format!("{fraction:.4}"))
            .unwrap_or_else(|| panic!("'{on_resize}' is not a String-payload Action variant"));
        self.dispatch(msg);
    }

    /// `<Slider>`: set the value through its `onChange` handler — the
    /// deterministic counterpart of the shell's track drag (same clamp,
    /// same `{:.4}` formatting).
    pub fn set_slider(&mut self, test_id: &str, value: f32) {
        use libra_components::ItemKind;
        let on_change = self
            .laid
            .items
            .iter()
            .find_map(|i| match &i.kind {
                ItemKind::SliderCtl { on_change, .. }
                    if i.semantics.test_id.as_deref() == Some(test_id) =>
                {
                    Some(on_change.clone())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("set_slider: no <Slider testId=\"{test_id}\">"));
        let value = value.clamp(0.0, 1.0);
        let msg = A::Msg::action_with_str(&on_change, format!("{value:.4}"))
            .unwrap_or_else(|| panic!("'{on_change}' is not a String-payload Action variant"));
        self.dispatch(msg);
    }

    /// §19.2: press a declared keyboard shortcut by its chord string —
    /// drives the app's own validated table with the same precedence
    /// the shell applies (a focused text control suppresses reserved
    /// chords). Panics on an unparseable chord, an invalid table, or a
    /// chord the app never declared; returns whether it *fired* (a
    /// declared chord suppressed by focus returns false).
    pub fn press_shortcut(&mut self, chord: &str) -> bool {
        let table = libra_core::build_shortcut_table::<A::Msg>(A::shortcuts())
            .unwrap_or_else(|e| panic!("press_shortcut: invalid shortcut table: {e}"));
        let want = libra_core::parse_chord(chord)
            .unwrap_or_else(|e| panic!("press_shortcut: {e}"));
        let (_, s) = table
            .iter()
            .find(|(c, _)| *c == want)
            .unwrap_or_else(|| panic!("press_shortcut: '{chord}' is not a declared shortcut"));
        if self.focus.is_some() && !libra_core::shortcut_fires_while_focused(&want) {
            return false;
        }
        let msg = match &s.arg {
            Some(arg) => A::Msg::action_with_str(&s.action, arg.clone()),
            None => A::Msg::action_by_name(&s.action),
        }
        .expect("table validated above");
        self.dispatch(msg);
        true
    }

    /// Render under a named theme (§11): `.theme("libra-light")`.
    /// Panics on unknown names — a typo'd theme is a broken test.
    pub fn theme(mut self, name: &str) -> Self {
        self.theme = Theme::by_name(name)
            .unwrap_or_else(|| panic!("unknown theme '{name}' — built in: libra-dark, libra-light"));
        self.laid = rebuild(&self.app, &self.ast, &self.renderer, self.viewport, &self.theme);
        self
    }

    /// Feed a recorded tape into a series, synchronously (§14.4a): every
    /// sample lands before this returns — no threads, no clocks. The
    /// deterministic input side of chart snapshot tests.
    pub fn feed_tape(&mut self, series: SeriesId, path: impl AsRef<Path>) -> usize {
        let n = self
            .store
            .feed_tape_sync(series, path.as_ref())
            .unwrap_or_else(|e| panic!("feed_tape: {e}"));
        // Data changed: rebuild so paint sees it (widgets read at paint,
        // but a rebuild keeps laid geometry current too).
        self.laid = rebuild(&self.app, &self.ast, &self.renderer, self.viewport, &self.theme);
        n
    }

    pub fn store(&self) -> &Arc<SeriesStore> {
        &self.store
    }

    pub fn state(&self) -> &A {
        &self.app
    }

    /// Dispatch an action directly through the real update, then
    /// rebuild. Returned effects are intercepted into the integration
    /// queue (§15.4).
    pub fn dispatch(&mut self, msg: A::Msg) {
        let effects = self.app.update(msg);
        for effect in &effects {
            // SetTheme is presentation state: apply it (snapshots see
            // it) *and* queue it (integration tests can assert on it).
            if let Effect::SetTheme { name } = effect {
                if let Some(t) = Theme::by_name(name) {
                    self.theme = t;
                }
            }
        }
        self.pending.extend(effects);
        self.laid = rebuild(&self.app, &self.ast, &self.renderer, self.viewport, &self.theme);
        // Deterministic layout inputs survive the rebuild (PLAN-SCROLL / M7).
        self.laid.apply_scroll(&self.scroll_state);
        self.laid.apply_menu_anchor(self.menu_anchor);
    }

    /// Hit-test a synthetic click at the center of the selected item and
    /// dispatch its handler through the app's real action resolution.
    /// Clicking a TextInput focuses it (§10.1) instead of dispatching.
    pub fn click(&mut self, sel: Selector) {
        let item = self.find(&sel).unwrap_or_else(|| panic!("click: no item matches {sel:?}"));
        let (cx, cy) = item.rect.center();
        let hit = self
            .laid
            .hit(cx, cy)
            .unwrap_or_else(|| panic!("click: {sel:?} is not interactive at ({cx},{cy})"));
        let name = hit.on_press.clone().expect("hit item has a handler");
        if hit.semantics.role == Role::TextInput {
            let id = hit.semantics.test_id.clone().expect("Input has a testId (load-enforced)");
            self.focus = Some((id, EditState::new(hit.semantics.text.clone()), name));
            return;
        }
        // Item-scoped handler (`onPress={toggle(item)}`, §6.2): dispatch
        // the String-payload action with the resolved loop-binding value;
        // otherwise a plain unit action by name.
        let msg = match hit.on_press_arg.clone() {
            Some(arg) => A::Msg::action_with_str(&name, arg).unwrap_or_else(|| {
                panic!("handler '{name}' resolved to no String-payload Action — reflection out of sync")
            }),
            None => A::Msg::action_by_name(&name).unwrap_or_else(|| {
                panic!("handler '{name}' resolved to no Action — reflection out of sync")
            }),
        };
        self.dispatch(msg);
    }

    // ------------------------------------------------------------------
    // Text entry (§10.1): the harness drives the same pure editing core
    // the shell drives.
    // ------------------------------------------------------------------

    /// Focus the selected Input and type, one char at a time — each
    /// keystroke runs the editing core and dispatches the input's
    /// onChange msg through the real update, exactly like the shell.
    pub fn type_text(&mut self, sel: Selector, text: &str) {
        self.click(sel);
        for ch in text.chars() {
            self.edit(EditOp::Insert(ch));
        }
    }

    /// Apply one editing op to the focused Input (§10.1 harness verb):
    /// text-changing ops dispatch onChange; navigation just moves the
    /// caret/selection.
    pub fn edit(&mut self, op: EditOp) {
        let Some((id, mut edit, on_change)) = self.focus.take() else {
            panic!("edit: no Input is focused — click or type_text first");
        };
        let before = edit.text().to_string();
        edit.apply(op);
        let after = edit.text().to_string();
        self.focus = Some((id.clone(), edit, on_change.clone()));
        if before != after {
            let msg = A::Msg::action_with_str(&on_change, after)
                .unwrap_or_else(|| panic!("'{on_change}' is not a String-payload Action variant"));
            self.dispatch(msg);
            // Controlled input: state is the source of truth. If
            // update transformed the text, resync the editing state.
            self.sync_focus_from_state();
        }
    }

    fn sync_focus_from_state(&mut self) {
        let Some((id, edit, on_change)) = self.focus.take() else { return };
        let current = self
            .find(&Selector::TestId(id.clone()))
            .map(|i| i.semantics.text.clone())
            .unwrap_or_default();
        let edit = if current == edit.text() {
            edit
        } else {
            EditState::new(current) // state rewrote the value: caret to end
        };
        self.focus = Some((id, edit, on_change));
    }

    /// The focused Input's editing state (caret/selection assertions).
    pub fn focused_edit(&self) -> Option<&EditState> {
        self.focus.as_ref().map(|(_, e, _)| e)
    }

    pub fn blur(&mut self) {
        self.focus = None;
    }

    fn focus_paint(&self) -> Option<InputFocus> {
        self.focus.as_ref().map(|(id, e, _)| InputFocus {
            test_id: id.clone(),
            caret: e.caret(),
            selection: e.selection(),
        })
    }

    // ------------------------------------------------------------------
    // Integration tests — effect interception & scripted results (§15.4)
    // ------------------------------------------------------------------

    /// Remove and return the first pending effect matching `matcher`.
    /// Panics (listing the queue) if none matches.
    pub fn expect_effect(&mut self, matcher: fx::Matcher) -> Effect<A::Msg> {
        let pos = self.pending.iter().position(|c| matches(&matcher, c));
        match pos {
            Some(i) => self.pending.remove(i).expect("position is valid"),
            None => panic!(
                "expect_effect: nothing matches {matcher:?}; pending queue: {:?}",
                self.pending
            ),
        }
    }

    /// Assert the effect queue is fully drained — the flow produced
    /// nothing the test didn't account for.
    pub fn expect_no_effects(&self) {
        assert!(
            self.pending.is_empty(),
            "expected no pending effects, found: {:?}",
            self.pending
        );
    }

    pub fn pending_effects(&self) -> impl Iterator<Item = &Effect<A::Msg>> {
        self.pending.iter()
    }

    /// Feed an HTTP fixture into an intercepted `Effect::Http` —
    /// constructs the action via the effect's own ctor and dispatches
    /// it for real.
    pub fn resolve_http(&mut self, effect: Effect<A::Msg>, result: HttpResult) {
        match effect {
            Effect::Http { on_done, .. } => self.dispatch(on_done(result)),
            other => panic!("resolve_http: not an Http effect: {other:?}"),
        }
    }

    pub fn resolve_kv_get(&mut self, effect: Effect<A::Msg>, value: Option<Vec<u8>>) {
        match effect {
            Effect::KvGet { on_done, .. } => self.dispatch(on_done(value)),
            other => panic!("resolve_kv_get: not a KvGet effect: {other:?}"),
        }
    }

    /// Acked sets dispatch their result; fire-and-forget sets are a no-op.
    pub fn resolve_kv_set(&mut self, effect: Effect<A::Msg>, result: KvResult) {
        match effect {
            Effect::KvSet { on_done: Some(ctor), .. } => self.dispatch(ctor(result)),
            Effect::KvSet { on_done: None, .. } => {}
            other => panic!("resolve_kv_set: not a KvSet effect: {other:?}"),
        }
    }

    pub fn resolve_kv_scan(&mut self, effect: Effect<A::Msg>, hits: Vec<(String, Vec<u8>)>) {
        match effect {
            Effect::KvScan { on_done, .. } => self.dispatch(on_done(hits)),
            other => panic!("resolve_kv_scan: not a KvScan effect: {other:?}"),
        }
    }

    /// Deliver an intercepted timer immediately — tests control time.
    pub fn fire_timer(&mut self, effect: Effect<A::Msg>) {
        match effect {
            Effect::Timer { action, .. } => self.dispatch(action),
            other => panic!("fire_timer: not a Timer effect: {other:?}"),
        }
    }

    /// Ack an intercepted `Effect::ClipboardSet` (PLAN-NATIVE-
    /// INTEGRATIONS) — no OS clipboard is touched in tests; the effect
    /// is the assertion surface, the result is hand-fed.
    pub fn resolve_clipboard(&mut self, effect: Effect<A::Msg>, result: libra_core::ClipboardResult) {
        match effect {
            Effect::ClipboardSet { on_done: Some(ctor), .. } => self.dispatch(ctor(result)),
            Effect::ClipboardSet { on_done: None, .. } => {}
            other => panic!("resolve_clipboard: not a ClipboardSet effect: {other:?}"),
        }
    }

    pub fn resolve_random(&mut self, effect: Effect<A::Msg>, bytes: Vec<u8>) {
        match effect {
            Effect::Random { on_done, .. } => self.dispatch(on_done(bytes)),
            other => panic!("resolve_random: not a Random effect: {other:?}"),
        }
    }

    // ------------------------------------------------------------------
    // Queries (component tests)
    // ------------------------------------------------------------------

    pub fn query(&self, sel: Selector) -> NodeRef<'_> {
        let item = self.find(&sel).unwrap_or_else(|| panic!("query: no item matches {sel:?}"));
        NodeRef { text: item.semantics.text.clone(), _life: std::marker::PhantomData }
    }

    fn find(&self, sel: &Selector) -> Option<&libra_components::Item> {
        self.laid.items.iter().find(|i| match sel {
            Selector::TestId(id) => i.semantics.test_id.as_deref() == Some(id.as_str()),
            Selector::Text(t) => i.semantics.text == *t,
            Selector::Role(r) => i.semantics.role == *r,
            Selector::Source(kind) => i.semantics.role == Role::Source && i.semantics.text == *kind,
        })
    }

    /// Is a source element of this kind (§12.5) currently mounted?
    /// Non-panicking, unlike `query` — the natural way to assert
    /// *absence* (e.g. a paused dashboard has no mounted `"frames"`).
    pub fn has_source(&self, kind: &str) -> bool {
        self.find(&Selector::Source(kind.to_string())).is_some()
    }

    // ------------------------------------------------------------------
    // Snapshots (snapshot tests)
    // ------------------------------------------------------------------

    /// Byte-exact snapshot assertion (see module docs). On mismatch,
    /// writes `<name>.actual.png` and `<name>.diff.png` (heatmap: red
    /// where pixels differ, dimmed expected elsewhere).
    pub fn assert_snapshot(&self, name: &str) {
        let cmds = self.laid.paint_full(&self.renderer, &self.store, self.focus_paint().as_ref(), self.hover, &self.theme, 1.0);
        let actual = self.renderer.render_png(&cmds, self.viewport.0, self.viewport.1);
        let path = self.snapshot_dir.join(format!("{name}.png"));

        if std::env::var("LIBRA_BLESS").is_ok() {
            panic!("LIBRA_BLESS was renamed — set LIBRA_VALIDATE=1 to accept new renderings");
        }
        let validate = std::env::var("LIBRA_VALIDATE").ok().as_deref() == Some("1");
        if validate || !path.exists() {
            std::fs::create_dir_all(&self.snapshot_dir).expect("snapshot dir");
            std::fs::write(&path, &actual).expect("write snapshot");
            if validate {
                eprintln!("[libra-test] validated snapshot '{name}' ({} bytes)", actual.len());
            } else {
                eprintln!("[libra-test] recorded new snapshot '{name}' ({} bytes)", actual.len());
            }
            return;
        }

        let expected = std::fs::read(&path).expect("read snapshot");
        if expected != actual {
            let actual_path = self.snapshot_dir.join(format!("{name}.actual.png"));
            std::fs::write(&actual_path, &actual).expect("write actual");
            let diff_note = match write_diff_heatmap(
                &expected,
                &actual,
                &self.snapshot_dir.join(format!("{name}.diff.png")),
            ) {
                Some((diff_path, changed)) => {
                    format!("\n  diff:     {} ({changed} px changed)", diff_path.display())
                }
                None => String::new(),
            };
            panic!(
                "snapshot '{name}' differs byte-for-byte\n  expected: {}\n  actual:   {}{diff_note}\n  (LIBRA_VALIDATE=1 to accept the new rendering)",
                path.display(),
                actual_path.display()
            );
        }
    }

    /// Raw PNG bytes of the current frame (for hashing/inspection).
    pub fn frame_png(&self) -> Vec<u8> {
        let cmds = self.laid.paint_full(&self.renderer, &self.store, self.focus_paint().as_ref(), self.hover, &self.theme, 1.0);
        self.renderer.render_png(&cmds, self.viewport.0, self.viewport.1)
    }
}

/// Failure artifact: red where pixels differ, dimmed expected elsewhere.
/// Returns the path and differing-pixel count, or None if either PNG
/// fails to decode or dimensions differ.
fn write_diff_heatmap(
    expected_png: &[u8],
    actual_png: &[u8],
    path: &std::path::Path,
) -> Option<(PathBuf, usize)> {
    let expected = tiny_skia::Pixmap::decode_png(expected_png).ok()?;
    let actual = tiny_skia::Pixmap::decode_png(actual_png).ok()?;
    if expected.width() != actual.width() || expected.height() != actual.height() {
        return None;
    }
    let mut diff = tiny_skia::Pixmap::new(expected.width(), expected.height())?;
    let (e, a, d) = (expected.pixels(), actual.pixels(), diff.pixels_mut());
    let mut changed = 0usize;
    for i in 0..e.len() {
        d[i] = if e[i] != a[i] {
            changed += 1;
            tiny_skia::PremultipliedColorU8::from_rgba(255, 0, 0, 255).expect("opaque red")
        } else {
            // Dimmed grayscale of expected, so the changed region reads
            // in context.
            let l = ((e[i].red() as u16 + e[i].green() as u16 + e[i].blue() as u16) / 12) as u8;
            tiny_skia::PremultipliedColorU8::from_rgba(l, l, l, 255).expect("opaque gray")
        };
    }
    let bytes = diff.encode_png().ok()?;
    std::fs::write(path, bytes).ok()?;
    Some((path.to_path_buf(), changed))
}

pub struct NodeRef<'a> {
    text: String,
    _life: std::marker::PhantomData<&'a ()>,
}

impl<'a> NodeRef<'a> {
    pub fn assert_text(&self, expected: &str) {
        assert_eq!(
            self.text, expected,
            "semantic text mismatch: expected {expected:?}, found {:?}",
            self.text
        );
    }
    pub fn text(&self) -> &str {
        &self.text
    }
}

fn rebuild<A: App>(
    app: &A,
    ast: &Node,
    renderer: &Renderer,
    viewport: (u32, u32),
    theme: &Theme,
) -> LaidTree {
    let field = |p: &str| app.field(p);
    let has_action = |m: &str| A::Msg::action_by_name(m).is_some();
    let has_str_action = |m: &str| A::Msg::action_with_str(m, String::new()).is_some();
    let collection_rows = |p: &str| app.collection_rows(p);
    let has_frame_info_action =
        |m: &str| A::Msg::action_with_frame_info(m, FrameInfo::at(0.0)).is_some();
    let has_instant_action = |m: &str| A::Msg::action_with_instant(m, Instant::at(0.0)).is_some();
    let scope = Scope {
        field: &field,
        has_action: &has_action,
        has_str_action: &has_str_action,
        collection_rows: &collection_rows,
        has_frame_info_action: &has_frame_info_action,
        has_instant_action: &has_instant_action,
    };
    let view = libra_interp::resolve(ast, &scope)
        .unwrap_or_else(|e| panic!("interp error: {e}"));
    libra_components::layout(&view, (viewport.0 as f32, viewport.1 as f32), renderer, theme)
        .unwrap_or_else(|e| panic!("{e}"))
}
