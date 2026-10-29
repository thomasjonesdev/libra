//! libra-test — the in-process `Harness<A: App>`: build the real view via the
//! real interpreter, query by semantics, and dispatch through real hit
//! testing and the app's real `update`. Effects are intercepted as data
//! (assert + hand-feed results in any order — no mocks); snapshots compare PNG
//! bytes exactly.
//!
//! Architecture: ARCHITECTURE.md §15 (testing architecture).

// ReflectState/ReflectAction methods resolve through App's supertrait bounds.
use libra_components::{
    caret_byte_at, Cursor, InputFocus, ItemKind, LaidTree, ScrollState, SourceDecl,
};
use libra_core::{
    App, Effect, FrameInfo, HttpResult, Instant, KvResult, ProcessResult, ReflectAction, SeriesId,
    StreamClose, StreamEvent,
};
use libra_data::SeriesStore;
use libra_draw::Theme;
use libra_interp::Scope;
use libra_markup::Node;
use libra_raster::Renderer;
use libra_text::{EditOp, EditState};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub mod autopilot;
pub mod respond;

pub use autopilot::{Affordance, AffordanceKind, Autopilot, Budget, Run, Step};
pub use libra_components::Role;
pub use respond::Reply;

/// Seed for the effect-drain shuffle and autopilot choices when a test does
/// not pick one. Fixed, so an unseeded run is still reproducible.
const DEFAULT_SEED: u64 = 0x_11B2A_0000_0001;

pub struct Harness<A: App> {
    app: A,
    ast: Node,
    registry: libra_markup::ComponentRegistry,
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
    /// How `drain_effects` answers intercepted effects, if the test set one
    /// (§15.4). `None` keeps the hand-fed behaviour every existing test uses.
    responder: Option<respond::Responder<A::Msg>>,
    /// Seeded PRNG behind effect-drain ordering and autopilot choices, so a
    /// failing run replays exactly from its seed.
    rng: respond::Rng,
    seed: u64,
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
        PickFolder,
        Notify,
        /// Match when `Notification.tag` equals this string.
        NotifyTag(String),
        WindowOp,
        LoginItem,
        LoginItemQuery,
        Process,
        /// Match when `CommandSpec.program` equals this string.
        ProcessProgram(Option<String>),
        Spawn,
        /// Match when `CommandSpec.program` equals this string.
        SpawnProgram(Option<String>),
        CloseStream,
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
    pub fn pick_folder() -> Matcher {
        Matcher::PickFolder
    }
    pub fn notify() -> Matcher {
        Matcher::Notify
    }
    pub fn notify_tag(tag: &str) -> Matcher {
        Matcher::NotifyTag(tag.to_string())
    }
    pub fn window_op() -> Matcher {
        Matcher::WindowOp
    }
    pub fn login_item() -> Matcher {
        Matcher::LoginItem
    }
    pub fn login_item_query() -> Matcher {
        Matcher::LoginItemQuery
    }
    pub fn process() -> Matcher {
        Matcher::Process
    }
    pub fn process_program(program: &str) -> Matcher {
        Matcher::ProcessProgram(Some(program.to_string()))
    }
    pub fn any_process() -> Matcher {
        Matcher::ProcessProgram(None)
    }
    pub fn spawn() -> Matcher {
        Matcher::Spawn
    }
    pub fn spawn_program(program: &str) -> Matcher {
        Matcher::SpawnProgram(Some(program.to_string()))
    }
    pub fn any_spawn() -> Matcher {
        Matcher::SpawnProgram(None)
    }
    pub fn close_stream() -> Matcher {
        Matcher::CloseStream
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
        (Matcher::PickFolder, Effect::PickFolder { .. }) => true,
        (Matcher::Notify, Effect::Notify { .. }) => true,
        (Matcher::NotifyTag(tag), Effect::Notify { note, .. }) => note.tag.as_deref() == Some(tag),
        (Matcher::WindowOp, Effect::WindowOp { .. }) => true,
        (Matcher::LoginItem, Effect::LoginItem { .. }) => true,
        (Matcher::LoginItemQuery, Effect::LoginItemQuery { .. }) => true,
        (Matcher::Process, Effect::Process { .. }) => true,
        (Matcher::ProcessProgram(want), Effect::Process { command, .. }) => {
            want.as_deref().map_or(true, |w| w == &command.program)
        }
        (Matcher::Spawn, Effect::Spawn { .. }) => true,
        (Matcher::SpawnProgram(want), Effect::Spawn { command, .. }) => {
            want.as_deref().map_or(true, |w| w == &command.program)
        }
        (Matcher::CloseStream, Effect::CloseStream { .. }) => true,
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
        Self::with_viewport_and_registry(lx_source, libra_markup::ComponentRegistry::new(), w, h)
    }

    /// Load composed components from a view directory (§7.2).
    pub fn with_view_dir(lx_source: &str, view_dir: &std::path::Path, w: u32, h: u32) -> Self {
        let registry = libra_markup::ComponentRegistry::load_from_dir(view_dir)
            .unwrap_or_else(|e| panic!("component registry: {e}"));
        Self::with_viewport_and_registry(lx_source, registry, w, h)
    }

    pub fn with_viewport_and_registry(
        lx_source: &str,
        registry: libra_markup::ComponentRegistry,
        w: u32,
        h: u32,
    ) -> Self {
        let ast =
            libra_markup::parse(lx_source).unwrap_or_else(|e| panic!("markup parse error: {e}"));
        let (app, init_effects) = A::init();
        let renderer = Renderer::new();
        let theme = Theme::dark();
        let laid = rebuild(&app, &ast, &registry, &renderer, (w, h), &theme);
        Harness {
            app,
            ast,
            registry,
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
            responder: None,
            rng: respond::Rng::new(DEFAULT_SEED),
            seed: DEFAULT_SEED,
        }
    }

    /// M7: right-click the selected row — pins the menu anchor to the
    /// item's center and dispatches its `onMenu` handler (item-scoped
    /// arg included), exactly like the shell's right-click.
    pub fn right_click(&mut self, sel: Selector) {
        use libra_components::ItemKind;
        let item = self
            .find(&sel)
            .unwrap_or_else(|| panic!("right_click: no item matches {sel:?}"));
        let ItemKind::ListItem {
            on_menu: Some((name, arg)),
            ..
        } = &item.kind
        else {
            panic!("right_click: {sel:?} has no onMenu handler");
        };
        let (name, arg) = (name.clone(), arg.clone());
        self.menu_anchor = item.rect.center();
        let msg = match arg {
            Some(a) => A::Msg::action_with_str(&name, a),
            None => A::Msg::action_by_name(&name),
        }
        .unwrap_or_else(|| {
            panic!("onMenu '{name}' resolved to no Action — reflection out of sync")
        });
        self.dispatch(msg);
    }

    /// Scroll a `<Scroll testId=…>` viewport by `dy` logical px (down = +).
    /// Clamped at the top by 0 and at the bottom by content overflow — a
    /// deterministic input, so scroll snapshots stay byte-exact (PLAN-SCROLL).
    pub fn scroll(&mut self, test_id: &str, dy: f32) {
        let pos = self.scroll_state.entry(test_id.to_string()).or_insert(0.0);
        *pos = (*pos + dy).max(0.0);
        self.laid = rebuild(
            &self.app,
            &self.ast,
            &self.registry,
            &self.renderer,
            self.viewport,
            &self.theme,
        );
        self.laid.apply_scroll(&self.scroll_state);
        self.laid.apply_menu_anchor(self.menu_anchor);
    }

    /// Pin the hover point to the selected item's center (§7.1
    /// ListItem hover wash) — an explicit input, so hover snapshots
    /// stay byte-exact. `blur_hover()` clears it.
    pub fn hover(&mut self, sel: Selector) {
        let item = self
            .find(&sel)
            .unwrap_or_else(|| panic!("hover: no item matches {sel:?}"));
        self.hover = Some(item.rect.center());
    }

    pub fn blur_hover(&mut self) {
        self.hover = None;
    }

    /// The pointer affordance over the selected item's center (§19) — what
    /// the OS cursor becomes there in a real window. Derived from the tree,
    /// so it is a component-level assertion (§15.3): the cursor is an OS
    /// resource and never reaches a pixel, which is why snapshots can't
    /// cover it and this verb exists.
    pub fn cursor_over(&self, sel: Selector) -> Cursor {
        let item = self
            .find(&sel)
            .unwrap_or_else(|| panic!("cursor_over: no item matches {sel:?}"));
        let (x, y) = item.rect.center();
        self.laid.cursor_at(x, y)
    }

    /// `cursor_over` at an explicit logical point — for affordances with no
    /// queryable identity of their own (a `<Split>` divider gutter) and for
    /// asserting empty space stays a plain arrow.
    pub fn cursor_at(&self, x: f32, y: f32) -> Cursor {
        self.laid.cursor_at(x, y)
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
        let want = libra_core::parse_chord(chord).unwrap_or_else(|e| panic!("press_shortcut: {e}"));
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

    /// §19.3: the app's current native menubar projection.
    pub fn menu(&self) -> libra_core::MenuDecl {
        self.app.menu()
    }

    /// §19.3: native context-menu projection after a right-click
    /// `onMenu` dispatch (`None` = painted `<ContextMenu>` path).
    pub fn context_menu(&self) -> Option<libra_core::MenuDecl> {
        self.app.context_menu()
    }

    /// Dispatch a native-menu action by reflected name. Searches the
    /// menubar, then the context menu. `arg` matches a String-payload
    /// item (`click_menu("openVault", Some(path))`).
    pub fn click_menu(&mut self, action: &str, arg: Option<&str>) {
        let bar = self.app.menu();
        let ctx = self.app.context_menu();
        let found = libra_core::find_menu_action(&bar, action, arg).is_some()
            || ctx
                .as_ref()
                .is_some_and(|d| libra_core::find_menu_action(d, action, arg).is_some());
        if !found {
            panic!(
                "click_menu: '{action}' arg={arg:?} is not an enabled item in menu() or context_menu()"
            );
        }
        let msg = match arg {
            Some(a) => A::Msg::action_with_str(action, a.to_string()),
            None => A::Msg::action_by_name(action),
        }
        .unwrap_or_else(|| {
            panic!("click_menu: '{action}' did not resolve — reflection out of sync")
        });
        self.dispatch(msg);
    }

    /// Render under a named theme (§11): `.theme("libra-light")`.
    /// Panics on unknown names — a typo'd theme is a broken test.
    pub fn theme(mut self, name: &str) -> Self {
        self.theme = Theme::by_name(name).unwrap_or_else(|| {
            panic!("unknown theme '{name}' — built in: libra-dark, libra-light")
        });
        self.laid = rebuild(
            &self.app,
            &self.ast,
            &self.registry,
            &self.renderer,
            self.viewport,
            &self.theme,
        );
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
        self.laid = rebuild(
            &self.app,
            &self.ast,
            &self.registry,
            &self.renderer,
            self.viewport,
            &self.theme,
        );
        n
    }

    pub fn store(&self) -> &Arc<SeriesStore> {
        &self.store
    }

    pub fn state(&self) -> &A {
        &self.app
    }

    /// Laid geometry for the current viewport. Overflow checks walk this.
    pub fn laid(&self) -> &LaidTree {
        &self.laid
    }

    pub fn set_viewport(&mut self, w: u32, h: u32) {
        self.viewport = (w, h);
        self.laid = rebuild(
            &self.app,
            &self.ast,
            &self.registry,
            &self.renderer,
            self.viewport,
            &self.theme,
        );
        self.laid.apply_scroll(&self.scroll_state);
        self.laid.apply_menu_anchor(self.menu_anchor);
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
            // Samples are ring data, like the effects runtime. Paint
            // reads the store, so a chart snapshot sees the point.
            if let Effect::Sample {
                series,
                t_ms,
                value,
            } = effect
            {
                self.store.ring(*series).push(libra_data::Sample {
                    t_ms: *t_ms,
                    v: *value,
                });
            }
        }
        self.pending.extend(effects);
        self.laid = rebuild(
            &self.app,
            &self.ast,
            &self.registry,
            &self.renderer,
            self.viewport,
            &self.theme,
        );
        // Deterministic layout inputs survive the rebuild (PLAN-SCROLL / M7).
        self.laid.apply_scroll(&self.scroll_state);
        self.laid.apply_menu_anchor(self.menu_anchor);
    }

    /// Hit-test a synthetic click at the center of the selected item and
    /// dispatch its handler through the app's real action resolution.
    /// Clicking a TextInput focuses it (§10.1) instead of dispatching.
    pub fn click(&mut self, sel: Selector) {
        let idx = self
            .find_index(&sel)
            .unwrap_or_else(|| panic!("click: no item matches {sel:?}"));
        self.click_item(idx, &format!("{sel:?}"));
    }

    /// Click the item at this index in the current laid tree.
    ///
    /// `click` resolves a selector to the *first* match, which is the right
    /// default for a hand-written test naming one thing. The autopilot has
    /// already picked an exact item out of the tree and must drive that one —
    /// list rows routinely share a role and sometimes a label.
    pub(crate) fn click_item(&mut self, idx: usize, what: &str) {
        let (cx, cy) = self.laid.items[idx].rect.center();
        let hit = self
            .laid
            .hit(cx, cy)
            .unwrap_or_else(|| panic!("click: {what} is not interactive at ({cx},{cy})"));
        let name = hit.on_press.clone().expect("hit item has a handler");
        if hit.semantics.role == Role::TextInput {
            let id = hit
                .semantics
                .test_id
                .clone()
                .expect("Input has a testId (load-enforced)");
            let text = hit.semantics.text.clone();
            let focus = self.focus.as_ref().map(|(fid, e, _)| InputFocus {
                test_id: fid.clone(),
                caret: e.caret(),
                selection: e.selection(),
            });
            let byte = caret_byte_at(hit, cx, cy, &self.renderer, &self.theme, focus.as_ref())
                .unwrap_or(text.len());
            let already = self
                .focus
                .as_ref()
                .is_some_and(|(fid, e, _)| fid == &id && e.text() == text);
            let switching = self.focus.as_ref().is_some_and(|(fid, _, _)| fid != &id);
            if switching {
                self.dispatch_focused_submit();
            }
            if !already {
                self.focus = Some((id, EditState::new(text), name));
            }
            self.edit(EditOp::SetCaret {
                byte,
                select: false,
            });
            return;
        }
        let on_press_arg = hit.on_press_arg.clone();
        self.dispatch_focused_submit();
        self.focus = None;
        // Item-scoped handler (`onPress={toggle(item)}`, §6.2): dispatch
        // the String-payload action with the resolved loop-binding value;
        // otherwise a plain unit action by name.
        let msg = match on_press_arg {
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
        let idx = self
            .find_index(&sel)
            .unwrap_or_else(|| panic!("type_text: no item matches {sel:?}"));
        if self.laid.items[idx].semantics.role != Role::TextInput {
            panic!("type_text: {sel:?} is not a text field");
        }
        self.type_into_item(idx, text);
    }

    /// `type_text` against an exact index — see [`Harness::click_item`].
    pub(crate) fn type_into_item(&mut self, idx: usize, text: &str) {
        let item = &self.laid.items[idx];
        let id = item
            .semantics
            .test_id
            .clone()
            .expect("Input has a testId (load-enforced)");
        let name = item.on_press.clone().expect("text field has onChange");
        let current = item.semantics.text.clone();
        // Moving focus off a field submits it, exactly as `click` does. There
        // is no way to reach another field in a real window without a click or
        // a Tab, and both commit the one being left; letting a test hop
        // straight across would leave a draft the shell would have flushed.
        if self.focus.as_ref().is_some_and(|(fid, _, _)| fid != &id) {
            self.dispatch_focused_submit();
        }
        let item = &self.laid.items[idx];
        let current = if item.semantics.test_id.as_deref() == Some(id.as_str()) {
            item.semantics.text.clone()
        } else {
            current
        };
        self.focus = Some((id, EditState::new(current), name));
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
        let Some((id, edit, on_change)) = self.focus.take() else {
            return;
        };
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

    /// `testId` of the focused text field, if one has focus. Focus is a
    /// deterministic input the harness owns rather than app state, so an
    /// assertion that depends on "is the user editing this?" has to read it
    /// from here.
    pub fn focused_test_id(&self) -> Option<&str> {
        self.focus.as_ref().map(|(id, _, _)| id.as_str())
    }

    pub fn blur(&mut self) {
        self.dispatch_focused_submit();
        self.focus = None;
    }

    fn dispatch_focused_submit(&mut self) {
        let Some((id, ..)) = &self.focus else {
            return;
        };
        let item = self
            .laid
            .items
            .iter()
            .find(|i| i.semantics.test_id.as_deref() == Some(id.as_str()));
        let Some(item) = item else {
            return;
        };
        let name = match &item.kind {
            libra_components::ItemKind::Search {
                on_submit: Some(name),
                ..
            } => name.clone(),
            libra_components::ItemKind::Input {
                on_submit: Some(name),
                ..
            } => name.clone(),
            _ => return,
        };
        let msg = A::Msg::action_by_name(&name)
            .unwrap_or_else(|| panic!("'{name}' is not a unit Action variant"));
        self.dispatch(msg);
    }

    /// Enter on the focused field (§7.1 Search/Input): dispatches `onSubmit`
    /// when declared; inserts a newline in a Textarea.
    pub fn press_enter(&mut self) {
        let Some((id, _, _)) = &self.focus else {
            panic!("press_enter: no Input is focused — click or type_text first");
        };
        let item = self
            .laid
            .items
            .iter()
            .find(|i| i.semantics.test_id.as_deref() == Some(id.as_str()))
            .unwrap_or_else(|| panic!("press_enter: focused '{id}' is gone from the tree"));
        match &item.kind {
            libra_components::ItemKind::Search {
                on_submit: Some(name),
                ..
            }
            | libra_components::ItemKind::Input {
                on_submit: Some(name),
                ..
            } => {
                let msg = A::Msg::action_by_name(name)
                    .unwrap_or_else(|| panic!("'{name}' is not a unit Action variant"));
                self.dispatch(msg);
            }
            libra_components::ItemKind::Textarea { .. } => {
                self.edit(EditOp::Insert('\n'));
            }
            _ => {}
        }
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
            Effect::KvSet {
                on_done: Some(ctor),
                ..
            } => self.dispatch(ctor(result)),
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
    pub fn resolve_clipboard(
        &mut self,
        effect: Effect<A::Msg>,
        result: libra_core::ClipboardResult,
    ) {
        match effect {
            Effect::ClipboardSet {
                on_done: Some(ctor),
                ..
            } => self.dispatch(ctor(result)),
            Effect::ClipboardSet { on_done: None, .. } => {}
            other => panic!("resolve_clipboard: not a ClipboardSet effect: {other:?}"),
        }
    }

    /// Dispatch the click action stored on an intercepted `Effect::Notify`
    /// (PLAN-NOTIFY NT2). No banner is shown.
    pub fn activate_notification(&mut self, effect: Effect<A::Msg>) {
        match effect {
            Effect::Notify {
                on_activate: Some(msg),
                ..
            } => self.dispatch(msg),
            Effect::Notify {
                on_activate: None, ..
            } => panic!("activate_notification: effect has no on_activate"),
            other => panic!("activate_notification: not a Notify effect: {other:?}"),
        }
    }

    /// The app's current tray projection (PLAN-RESIDENCY R2).
    pub fn tray(&self) -> Option<libra_core::TrayDecl> {
        self.app.tray()
    }

    /// Dispatch a tray action by reflected name. Panics when the item is
    /// missing or disabled.
    pub fn click_tray(&mut self, action: &str) {
        let Some(decl) = self.app.tray() else {
            panic!("click_tray: '{action}' — the app has no tray");
        };
        if let Err(e) = libra_core::tray_clickable(&decl, action) {
            panic!("{e}");
        }
        let msg = A::Msg::action_by_name(action).unwrap_or_else(|| {
            panic!("click_tray: '{action}' did not resolve — reflection out of sync")
        });
        self.dispatch(msg);
    }

    /// Ack an intercepted `Effect::Notify` (PLAN-NOTIFY NT1). No banner
    /// is shown. Headless default, when a test does not feed a result
    /// through the effects runtime, is `Unsupported("headless")`.
    pub fn resolve_notify(&mut self, effect: Effect<A::Msg>, result: libra_core::NotifyResult) {
        match effect {
            Effect::Notify {
                on_done: Some(ctor),
                ..
            } => self.dispatch(ctor(result)),
            Effect::Notify { on_done: None, .. } => {}
            other => panic!("resolve_notify: not a Notify effect: {other:?}"),
        }
    }

    pub fn resolve_login_item(
        &mut self,
        effect: Effect<A::Msg>,
        result: libra_core::LoginItemResult,
    ) {
        match effect {
            Effect::LoginItem { on_done, .. } | Effect::LoginItemQuery { on_done } => {
                self.dispatch(on_done(result))
            }
            other => panic!("resolve_login_item: not a login-item effect: {other:?}"),
        }
    }

    pub fn resolve_folder_pick(&mut self, effect: Effect<A::Msg>, result: libra_core::FolderPick) {
        match effect {
            Effect::PickFolder { on_done } => self.dispatch(on_done(result)),
            other => panic!("resolve_folder_pick: not a PickFolder effect: {other:?}"),
        }
    }

    pub fn resolve_random(&mut self, effect: Effect<A::Msg>, bytes: Vec<u8>) {
        match effect {
            Effect::Random { on_done, .. } => self.dispatch(on_done(bytes)),
            other => panic!("resolve_random: not a Random effect: {other:?}"),
        }
    }

    /// Deliver one stream event to the mounted `<Stream id>` handler.
    /// Panics when that id is not mounted — a test must not lose a line
    /// the way a forgotten handler would.
    pub fn emit_stream(&mut self, id: &str, event: StreamEvent) {
        let handler = self
            .stream_handler(id)
            .unwrap_or_else(|| panic!("emit_stream: no <Stream id={id}> is mounted"));
        let msg = A::Msg::action_with_stream_event(&handler, event)
            .unwrap_or_else(|| panic!("emit_stream: '{handler}' is not a StreamEvent action"));
        self.dispatch(msg);
    }

    /// `emit_stream` of [`StreamEvent::Closed`].
    pub fn close_stream(&mut self, id: &str, close: StreamClose) {
        self.emit_stream(id, StreamEvent::Closed(close));
    }

    fn stream_handler(&self, id: &str) -> Option<String> {
        let markup: Vec<(String, String)> = self
            .laid
            .items
            .iter()
            .filter_map(|item| match &item.kind {
                ItemKind::Source(SourceDecl::Stream { id, on_event }) => {
                    Some((id.clone(), on_event.clone()))
                }
                _ => None,
            })
            .collect();
        let ambient: Vec<(String, String)> = self
            .app
            .subscriptions()
            .into_iter()
            .filter_map(|sub| match sub {
                libra_core::Sub::Stream { id, on_event, .. } => Some((id, on_event)),
                libra_core::Sub::Frames { .. } => None,
            })
            .collect();
        libra_core::union_stream_handlers(&markup, &ambient)
            .into_iter()
            .find(|(sid, _)| sid == id)
            .map(|(_, handler)| handler)
    }

    /// Feed a process fixture into an intercepted `Effect::Process`.
    pub fn resolve_process(&mut self, effect: Effect<A::Msg>, result: ProcessResult) {
        match effect {
            Effect::Process { on_done, .. } => self.dispatch(on_done(result)),
            other => panic!("resolve_process: not a Process effect: {other:?}"),
        }
    }

    // ------------------------------------------------------------------
    // Automatic effect resolution (§15.4 + §12.3)
    // ------------------------------------------------------------------

    /// Answer intercepted effects from a fixture function instead of one
    /// `resolve_*` call per effect, so a run can take steps whose effects it
    /// cannot enumerate in advance. Results are still fixtures — this executes
    /// nothing (§12.6).
    ///
    /// ```ignore
    /// let mut h = Harness::<Gloss>::render(gloss::VIEW).responder(|fx| match fx {
    ///     Effect::Process { command, .. } if command.args.first() == Some(&"list".into()) => {
    ///         Reply::Process(ok_stdout("Atlas.md\n"))
    ///     }
    ///     Effect::Process { .. } => Reply::Process(ok_stdout("# note\n")),
    ///     Effect::KvGet { .. } => Reply::KvGet(None),
    ///     _ => Reply::Drop,
    /// });
    /// ```
    pub fn responder(mut self, f: impl FnMut(&Effect<A::Msg>) -> Reply + 'static) -> Self {
        self.responder = Some(Box::new(f));
        self
    }

    /// Seed the effect-drain shuffle and every autopilot choice. Two runs with
    /// the same seed are the same run — that is what makes a failure a bug
    /// report rather than a flake.
    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self.rng = respond::Rng::new(seed);
        self
    }

    /// Drive the app for `steps` interactions, picking affordances out of the
    /// real tree (§15.7). See [`autopilot`] for what a run does and asserts.
    ///
    /// ```ignore
    /// h.autopilot(400)
    ///     .avoid_handlers(["openVaultPicker"])
    ///     .invariant("settles", |s| s.io == IoKind::Idle)
    ///     .bounded("history", 500, |s| s.history_back.len())
    ///     .budget(Budget::new().max_ms(40.0))
    ///     .run();
    /// ```
    pub fn autopilot(&mut self, steps: usize) -> Autopilot<'_, A> {
        Autopilot::new(self, steps)
    }

    pub(crate) fn seed_value(&self) -> u64 {
        self.seed
    }

    /// Next autopilot choice below `n`.
    pub(crate) fn pick(&mut self, n: usize) -> usize {
        self.rng.below(n)
    }

    /// Resolve pending effects through the responder until the queue stops
    /// making progress, and return how many were delivered.
    ///
    /// Each round shuffles the queue before answering it. §12.3 leaves
    /// cross-effect ordering unguaranteed, so emission order is the one order
    /// production will never promise; draining in it exclusively would test
    /// the single arrangement that cannot be relied on.
    ///
    /// Panics if the queue never settles — an effect cycle is a real bug, and
    /// a test that hangs on one reports nothing.
    pub fn drain_effects(&mut self) -> usize {
        let mut delivered = 0usize;
        for _ in 0..respond::MAX_ROUNDS {
            if self.pending.is_empty() {
                return delivered;
            }
            let (n, progressed) = self.drain_one_round();
            delivered += n;
            // Everything left is deferred and nothing moved: as settled as
            // this responder will get.
            if !progressed {
                return delivered;
            }
        }
        panic!(
            "drain_effects: still emitting after {} rounds — effect cycle?\n  queued: {}",
            respond::MAX_ROUNDS,
            self.pending
                .iter()
                .map(respond::label)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    /// Answer one round of pending effects and stop, leaving whatever those
    /// answers emitted still in flight.
    ///
    /// This is how a test reaches states that only exist *between* a request
    /// and its result. Draining to quiescence after every interaction models a
    /// user who waits for each spinner; real users click again first, and an
    /// app whose next action reads state the in-flight result was going to
    /// overwrite only misbehaves for them.
    pub fn drain_round(&mut self) -> usize {
        self.drain_one_round().0
    }

    /// Returns how many were delivered, and whether anything moved at all.
    fn drain_one_round(&mut self) -> (usize, bool) {
        let mut delivered = 0usize;
        let mut progressed = false;
        let mut slots: Vec<Option<Effect<A::Msg>>> = self.pending.drain(..).map(Some).collect();
        let mut order: Vec<usize> = (0..slots.len()).collect();
        self.rng.shuffle(&mut order);
        for i in order {
            let Some(effect) = slots[i].take() else {
                continue;
            };
            let reply = match &mut self.responder {
                Some(f) => f(&effect),
                None => Reply::Defer,
            };
            match reply {
                Reply::Defer => self.pending.push_back(effect),
                Reply::Drop => progressed = true,
                reply => {
                    self.deliver(effect, reply);
                    delivered += 1;
                    progressed = true;
                }
            }
        }
        (delivered, progressed)
    }

    fn deliver(&mut self, effect: Effect<A::Msg>, reply: Reply) {
        match (&effect, reply) {
            (Effect::Http { .. }, Reply::Http(r)) => self.resolve_http(effect, r),
            (Effect::KvGet { .. }, Reply::KvGet(v)) => self.resolve_kv_get(effect, v),
            (Effect::KvSet { .. }, Reply::KvSet(r)) => self.resolve_kv_set(effect, r),
            (Effect::KvScan { .. }, Reply::KvScan(hits)) => self.resolve_kv_scan(effect, hits),
            (Effect::Random { .. }, Reply::Random(b)) => self.resolve_random(effect, b),
            (Effect::Process { .. }, Reply::Process(r)) => self.resolve_process(effect, r),
            (Effect::ClipboardSet { .. }, Reply::Clipboard(r)) => self.resolve_clipboard(effect, r),
            (Effect::PickFolder { .. }, Reply::FolderPick(p)) => {
                self.resolve_folder_pick(effect, p)
            }
            (Effect::Notify { .. }, Reply::Notify(r)) => self.resolve_notify(effect, r),
            (Effect::Timer { .. }, Reply::Timer) => self.fire_timer(effect),
            (effect, reply) => panic!(
                "responder answered {} with {reply:?} — wrong reply kind",
                respond::label(effect)
            ),
        }
    }

    // ------------------------------------------------------------------
    // Queries (component tests)
    // ------------------------------------------------------------------

    pub fn query(&self, sel: Selector) -> NodeRef<'_> {
        let item = self
            .find(&sel)
            .unwrap_or_else(|| panic!("query: no item matches {sel:?}"));
        NodeRef {
            text: item.semantics.text.clone(),
            _life: std::marker::PhantomData,
        }
    }

    /// Laid-out box of the first match, `(x, y, width, height)` in logical px.
    pub fn rect(&self, sel: Selector) -> (f32, f32, f32, f32) {
        let item = self
            .find(&sel)
            .unwrap_or_else(|| panic!("rect: no item matches {sel:?}"));
        let r = item.rect;
        (r.x, r.y, r.w, r.h)
    }

    fn find(&self, sel: &Selector) -> Option<&libra_components::Item> {
        self.find_index(sel).map(|i| &self.laid.items[i])
    }

    fn find_index(&self, sel: &Selector) -> Option<usize> {
        self.laid.items.iter().position(|i| match sel {
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
        let cmds = self.laid.paint_full(
            &self.renderer,
            &self.store,
            self.focus_paint().as_ref(),
            self.hover,
            &self.theme,
            1.0,
        );
        let actual = self
            .renderer
            .render_png(&cmds, self.viewport.0, self.viewport.1);
        let path = self.snapshot_dir.join(format!("{name}.png"));

        if std::env::var("LIBRA_BLESS").is_ok() {
            panic!("LIBRA_BLESS was renamed — set LIBRA_VALIDATE=1 to accept new renderings");
        }
        let validate = std::env::var("LIBRA_VALIDATE").ok().as_deref() == Some("1");
        if validate || !path.exists() {
            std::fs::create_dir_all(&self.snapshot_dir).expect("snapshot dir");
            std::fs::write(&path, &actual).expect("write snapshot");
            if validate {
                eprintln!(
                    "[libra-test] validated snapshot '{name}' ({} bytes)",
                    actual.len()
                );
            } else {
                eprintln!(
                    "[libra-test] recorded new snapshot '{name}' ({} bytes)",
                    actual.len()
                );
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
                    format!(
                        "\n  diff:     {} ({changed} px changed)",
                        diff_path.display()
                    )
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
        let cmds = self.laid.paint_full(
            &self.renderer,
            &self.store,
            self.focus_paint().as_ref(),
            self.hover,
            &self.theme,
            1.0,
        );
        self.renderer
            .render_png(&cmds, self.viewport.0, self.viewport.1)
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
    registry: &libra_markup::ComponentRegistry,
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
    let has_stream_event_action =
        |m: &str| A::Msg::action_with_stream_event(m, StreamEvent::Dropped(0)).is_some();
    let scope = Scope {
        field: &field,
        has_action: &has_action,
        has_str_action: &has_str_action,
        collection_rows: &collection_rows,
        has_frame_info_action: &has_frame_info_action,
        has_instant_action: &has_instant_action,
        has_stream_event_action: &has_stream_event_action,
        binding_ctx: "State",
    };
    let view = libra_interp::resolve(ast, &scope, registry)
        .unwrap_or_else(|e| panic!("interp error: {e}"));
    libra_components::layout(
        &view,
        (viewport.0 as f32, viewport.1 as f32),
        renderer,
        theme,
    )
    .unwrap_or_else(|e| panic!("{e}"))
}
