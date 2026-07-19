//! The Phase 1 reference app: a counter that fetches and persists — everything
//! the effects runtime does, exercised through the pure loop (KV load/save on
//! `init:` and on every count change, plus an HTTP quote into a
//! `Remote<String, String>`). `quote_state` is `#[libra(skip)]`ped (a `Remote`,
//! not Display); the plain `quote` field is its display projection.
//!
//! Architecture: ARCHITECTURE.md §12 (effects runtime), §5.4 (actions are
//! events), §6.3 (display projections).

use libra::{actions, app, state};
use libra_core::{Effect, HttpRequest, HttpResult, KvResult, Remote};

pub const COUNT_KEY: &str = "fetcher/count";
pub const ZEN_URL: &str = "https://api.github.com/zen";

#[state]
pub struct Fetcher {
    pub count: i64,
    /// Display projection of `quote_state` (§6.3).
    pub quote: String,
    /// Last persistence outcome, as displayable state.
    pub save_status: String,
    #[libra(skip)]
    pub quote_state: Remote<String, String>,
}

impl Fetcher {
    pub fn init() -> (Fetcher, Vec<Effect<Action>>) {
        let mut state = Fetcher {
            count: 0,
            quote: String::new(),
            save_status: "not saved yet".into(),
            quote_state: Remote::NotAsked,
        };
        state.set_quote_state(Remote::NotAsked);
        (state, vec![Effect::kv_get(COUNT_KEY, Action::CountLoaded)])
    }

    fn set_quote_state(&mut self, state: Remote<String, String>) {
        self.quote = match &state {
            Remote::NotAsked => "press fetch".into(),
            Remote::Loading => "fetching…".into(),
            Remote::Success(q) => q.clone(),
            Remote::Failure(e) => format!("failed: {e}"),
        };
        self.quote_state = state;
    }

    fn persist(&self) -> Vec<Effect<Action>> {
        vec![Effect::kv_set_acked(COUNT_KEY, self.count.to_le_bytes().to_vec(), Action::Saved)]
    }
}

#[actions]
impl Fetcher {
    pub fn increment(&mut self) -> Vec<Effect<Action>> {
        self.count += 1;
        self.persist()
    }
    pub fn decrement(&mut self) -> Vec<Effect<Action>> {
        self.count -= 1;
        self.persist()
    }
    pub fn reset(&mut self) -> Vec<Effect<Action>> {
        self.count = 0;
        self.persist()
    }
    pub fn fetch_quote(&mut self) -> Effect<Action> {
        self.set_quote_state(Remote::Loading);
        Effect::http(HttpRequest::get(ZEN_URL), Action::QuoteArrived)
    }

    // Continuations (§5.2): effect-result payloads, not markup handlers.
    fn count_loaded(&mut self, bytes: Option<Vec<u8>>) {
        let Some(bytes) = bytes else { return }; // first run: keep defaults
        if let Ok(eight) = <[u8; 8]>::try_from(bytes.as_slice()) {
            self.count = i64::from_le_bytes(eight);
            self.save_status = "restored".into();
        }
    }
    fn saved(&mut self, result: KvResult) {
        self.save_status = match result {
            KvResult::Ok => "saved".into(),
            KvResult::Err(e) => format!("save failed: {e}"),
        };
    }
    fn quote_arrived(&mut self, result: HttpResult) {
        match result {
            HttpResult::Response { status: 200, body } => {
                self.set_quote_state(Remote::Success(String::from_utf8_lossy(&body).into_owned()));
            }
            HttpResult::Response { status, .. } => {
                self.set_quote_state(Remote::Failure(format!("HTTP {status}")));
            }
            HttpResult::Failed(e) => self.set_quote_state(Remote::Failure(e)),
        }
    }
}

app! {
    state: Fetcher,
    view: "../views/fetcher.lx",
    init: Fetcher::init,
    window: (460.0, 260.0),
    title: "Libra Fetcher",
}
