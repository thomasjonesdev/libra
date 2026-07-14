//! The Phase 1 reference app: the counter that fetches and persists
//! (ARCHITECTURE.md §23, Phase 1). Everything the effects runtime can
//! do, exercised through the pure loop:
//!
//! - startup loads the persisted count (`Cmd::KvGet` from `init`)
//! - every count change persists (`Cmd::KvSet`, acked)
//! - "Fetch zen" GETs a plain-text quote into a `Remote<String, String>`
//!
//! Msgs are events (§5.3): `CountLoaded`, `Saved`, `QuoteArrived` — what
//! happened, not what to do.
//!
//! Reflection note: `quote_state` is a `Remote` (not Display), so it is
//! `#[libra(skip)]`ped; its display projection is the plain `quote`
//! field, maintained in `update`. That's §6.3's rule — if you want to
//! compute it, put it on the Model — applied to reflection.

use libra_core::{App, Cmd, HttpRequest, HttpResult, KvResult, LibraReflect, Remote};
use serde::{Deserialize, Serialize};

pub const VIEW: &str = include_str!("../views/fetcher.lx");
pub const COUNT_KEY: &str = "fetcher/count";
pub const ZEN_URL: &str = "https://api.github.com/zen";

#[derive(Debug, Clone, PartialEq, LibraReflect, Serialize, Deserialize)]
pub struct Fetcher {
    pub count: i64,
    /// Display projection of `quote_state` (§6.3).
    pub quote: String,
    /// Last persistence outcome, as displayable state.
    pub save_status: String,
    #[libra(skip)]
    pub quote_state: Remote<String, String>,
}

#[derive(Debug, Clone, PartialEq, LibraReflect)]
pub enum Msg {
    Increment,
    Decrement,
    Reset,
    FetchQuote,
    CountLoaded(Option<Vec<u8>>),
    Saved(KvResult),
    QuoteArrived(HttpResult),
}

fn persist(count: i64) -> Vec<Cmd<Msg>> {
    vec![Cmd::kv_set_acked(COUNT_KEY, count.to_le_bytes().to_vec(), Msg::Saved)]
}

impl Fetcher {
    fn set_quote_state(&mut self, state: Remote<String, String>) {
        self.quote = match &state {
            Remote::NotAsked => "press fetch".into(),
            Remote::Loading => "fetching…".into(),
            Remote::Success(q) => q.clone(),
            Remote::Failure(e) => format!("failed: {e}"),
        };
        self.quote_state = state;
    }
}

impl App for Fetcher {
    type Msg = Msg;

    fn init() -> (Self, Vec<Cmd<Msg>>) {
        let mut model = Fetcher {
            count: 0,
            quote: String::new(),
            save_status: "not saved yet".into(),
            quote_state: Remote::NotAsked,
        };
        model.set_quote_state(Remote::NotAsked);
        (model, vec![Cmd::kv_get(COUNT_KEY, Msg::CountLoaded)])
    }

    fn update(&mut self, msg: Msg) -> Vec<Cmd<Msg>> {
        match msg {
            Msg::Increment => {
                self.count += 1;
                persist(self.count)
            }
            Msg::Decrement => {
                self.count -= 1;
                persist(self.count)
            }
            Msg::Reset => {
                self.count = 0;
                persist(self.count)
            }
            Msg::FetchQuote => {
                self.set_quote_state(Remote::Loading);
                vec![Cmd::http(HttpRequest::get(ZEN_URL), Msg::QuoteArrived)]
            }
            Msg::CountLoaded(Some(bytes)) => {
                if let Ok(eight) = <[u8; 8]>::try_from(bytes.as_slice()) {
                    self.count = i64::from_le_bytes(eight);
                    self.save_status = "restored".into();
                }
                vec![]
            }
            Msg::CountLoaded(None) => vec![], // first run: keep defaults
            Msg::Saved(KvResult::Ok) => {
                self.save_status = "saved".into();
                vec![]
            }
            Msg::Saved(KvResult::Err(e)) => {
                self.save_status = format!("save failed: {e}");
                vec![]
            }
            Msg::QuoteArrived(HttpResult::Response { status: 200, body }) => {
                self.set_quote_state(Remote::Success(
                    String::from_utf8_lossy(&body).into_owned(),
                ));
                vec![]
            }
            Msg::QuoteArrived(HttpResult::Response { status, .. }) => {
                self.set_quote_state(Remote::Failure(format!("HTTP {status}")));
                vec![]
            }
            Msg::QuoteArrived(HttpResult::Failed(e)) => {
                self.set_quote_state(Remote::Failure(e));
                vec![]
            }
        }
    }
}
