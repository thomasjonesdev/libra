//! libra-query — the query cache (Appendix C, via TanStack Query):
//! request dedup, stale-while-revalidate, retry with backoff, and
//! invalidation — implemented as **pure model machinery**. A `Query<T>`
//! lives on the Model; every decision is a pure function of
//! (query state, injected time), every effect is a returned `Cmd`, so
//! the whole policy is Tier-1/Tier-3 testable with zero infrastructure.
//!
//! Shape of use:
//! ```ignore
//! // in update():
//! Msg::ScreenOpened(now) => self.user.request(now.t_ms, &POLICY, req(), Msg::UserArrived),
//! Msg::UserArrived(r)    => self.user.resolve(now_from(&r), &POLICY, r, parse_user, Msg::RetryUser),
//! Msg::RetryUser         => self.user.retry(req(), Msg::UserArrived),
//! ```

use libra_core::{Cmd, HttpRequest, HttpResult, MsgCtor, Remote};

/// Caching/retry policy — a plain constant on the app side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QueryPolicy {
    /// Data younger than this is fresh: `request` is a no-op (dedup).
    pub stale_ms: f64,
    /// Failed fetches retry this many times…
    pub retries: u32,
    /// …starting at this backoff, doubling per attempt.
    pub backoff_ms: u64,
}

impl QueryPolicy {
    pub const DEFAULT: QueryPolicy = QueryPolicy { stale_ms: 30_000.0, retries: 3, backoff_ms: 500 };
}

/// Cached async data on the Model. `T` is the parsed payload.
#[derive(Debug, Clone, PartialEq)]
pub struct Query<T> {
    state: Remote<T, String>,
    /// Injected time of the last successful fetch (§5.2: no clocks).
    fetched_at_ms: Option<f64>,
    in_flight: bool,
    attempt: u32,
}

impl<T> Default for Query<T> {
    fn default() -> Self {
        Query { state: Remote::NotAsked, fetched_at_ms: None, in_flight: false, attempt: 0 }
    }
}

impl<T> Query<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare interest in the data. Returns the fetch cmd, or nothing
    /// if a request is already in flight (dedup) or the cache is still
    /// fresh. Stale-with-data keeps serving `data()` while refetching
    /// (SWR).
    #[must_use]
    pub fn request<M>(
        &mut self,
        now_ms: f64,
        policy: &QueryPolicy,
        req: HttpRequest,
        on_done: MsgCtor<HttpResult, M>,
    ) -> Vec<Cmd<M>> {
        if self.in_flight {
            return vec![]; // dedup: one fetch at a time per key
        }
        if let Some(at) = self.fetched_at_ms {
            if now_ms - at < policy.stale_ms && matches!(self.state, Remote::Success(_)) {
                return vec![]; // fresh: nothing to do
            }
        }
        self.in_flight = true;
        self.attempt = 0;
        if !matches!(self.state, Remote::Success(_)) {
            self.state = Remote::Loading;
        } // else: SWR — keep showing the stale data while refetching
        vec![Cmd::http(req, on_done)]
    }

    /// Feed the fetch result back. On success the payload parses via a
    /// pure `parse`; on failure a `Cmd::Timer` schedules `retry_msg`
    /// with exponential backoff until retries are exhausted.
    #[must_use]
    pub fn resolve<M>(
        &mut self,
        now_ms: f64,
        policy: &QueryPolicy,
        result: HttpResult,
        parse: fn(&[u8]) -> Result<T, String>,
        retry_msg: M,
    ) -> Vec<Cmd<M>> {
        let failure = match result {
            HttpResult::Response { status: 200, body } => match parse(&body) {
                Ok(value) => {
                    self.state = Remote::Success(value);
                    self.fetched_at_ms = Some(now_ms);
                    self.in_flight = false;
                    self.attempt = 0;
                    return vec![];
                }
                Err(e) => format!("parse: {e}"),
            },
            HttpResult::Response { status, .. } => format!("HTTP {status}"),
            HttpResult::Failed(e) => e,
        };
        if self.attempt < policy.retries {
            let after_ms = policy.backoff_ms << self.attempt; // 1×, 2×, 4×…
            self.attempt += 1;
            // Still in flight: dedup holds across the backoff window.
            return vec![Cmd::timer(after_ms, retry_msg)];
        }
        self.in_flight = false;
        if !matches!(self.state, Remote::Success(_)) {
            self.state = Remote::Failure(failure);
        } // else: SWR — a failed refresh keeps the last good data
        vec![]
    }

    /// The retry tick: re-issue the fetch (attempt counter persists).
    #[must_use]
    pub fn retry<M>(&mut self, req: HttpRequest, on_done: MsgCtor<HttpResult, M>) -> Vec<Cmd<M>> {
        vec![Cmd::http(req, on_done)]
    }

    /// Drop freshness so the next `request` refetches
    /// (`Cmd::Invalidate` semantics, model-side).
    pub fn invalidate(&mut self) {
        self.fetched_at_ms = None;
    }

    /// SWR read: the last good data, even while stale or refetching.
    pub fn data(&self) -> Option<&T> {
        match &self.state {
            Remote::Success(t) => Some(t),
            _ => None,
        }
    }
    pub fn error(&self) -> Option<&str> {
        match &self.state {
            Remote::Failure(e) => Some(e),
            _ => None,
        }
    }
    pub fn is_fetching(&self) -> bool {
        self.in_flight
    }
    pub fn state(&self) -> &Remote<T, String> {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY: QueryPolicy = QueryPolicy { stale_ms: 1000.0, retries: 2, backoff_ms: 100 };

    fn req() -> HttpRequest {
        HttpRequest::get("https://api.example.com/user")
    }
    fn parse_ok(b: &[u8]) -> Result<String, String> {
        Ok(String::from_utf8_lossy(b).into_owned())
    }

    #[derive(Debug, Clone, PartialEq)]
    enum Msg {
        Got(HttpResult),
        Retry,
    }

    fn ok(body: &str) -> HttpResult {
        HttpResult::Response { status: 200, body: body.as_bytes().to_vec() }
    }

    #[test]
    fn fresh_data_dedups_and_staleness_refetches() {
        let mut q: Query<String> = Query::new();
        assert_eq!(q.request(0.0, &POLICY, req(), Msg::Got).len(), 1);
        assert!(q.request(1.0, &POLICY, req(), Msg::Got).is_empty(), "in flight: dedup");
        assert!(q.resolve(10.0, &POLICY, ok("v1"), parse_ok, Msg::Retry).is_empty());
        assert_eq!(q.data().map(String::as_str), Some("v1"));

        assert!(q.request(500.0, &POLICY, req(), Msg::Got).is_empty(), "fresh: no refetch");
        assert_eq!(q.request(1500.0, &POLICY, req(), Msg::Got).len(), 1, "stale: refetch");
        assert_eq!(q.data().map(String::as_str), Some("v1"), "SWR: stale data still served");
        assert!(q.is_fetching());
    }

    #[test]
    fn retries_back_off_exponentially_then_fail() {
        let mut q: Query<String> = Query::new();
        let _ = q.request(0.0, &POLICY, req(), Msg::Got);

        let cmds = q.resolve(1.0, &POLICY, HttpResult::Failed("t/o".into()), parse_ok, Msg::Retry);
        assert_eq!(cmds, vec![Cmd::timer(100, Msg::Retry)], "1st retry at base backoff");
        let _ = q.retry(req(), Msg::Got);

        let cmds = q.resolve(2.0, &POLICY, HttpResult::Failed("t/o".into()), parse_ok, Msg::Retry);
        assert_eq!(cmds, vec![Cmd::timer(200, Msg::Retry)], "2nd retry doubles");
        let _ = q.retry(req(), Msg::Got);

        let cmds = q.resolve(3.0, &POLICY, HttpResult::Failed("t/o".into()), parse_ok, Msg::Retry);
        assert!(cmds.is_empty(), "retries exhausted");
        assert_eq!(q.error(), Some("t/o"));
        assert!(!q.is_fetching());
    }

    #[test]
    fn failed_refresh_keeps_last_good_data() {
        let mut q: Query<String> = Query::new();
        let _ = q.request(0.0, &POLICY, req(), Msg::Got);
        let _ = q.resolve(1.0, &POLICY, ok("good"), parse_ok, Msg::Retry);

        let _ = q.request(5000.0, &POLICY, req(), Msg::Got); // stale → refetch
        let mut r = HttpResult::Failed("down".into());
        for _ in 0..=POLICY.retries {
            let _ = q.resolve(5001.0, &POLICY, r.clone(), parse_ok, Msg::Retry);
            r = HttpResult::Failed("down".into());
        }
        assert_eq!(q.data().map(String::as_str), Some("good"), "SWR through total failure");
    }

    #[test]
    fn invalidate_forces_the_next_request_through() {
        let mut q: Query<String> = Query::new();
        let _ = q.request(0.0, &POLICY, req(), Msg::Got);
        let _ = q.resolve(1.0, &POLICY, ok("v1"), parse_ok, Msg::Retry);
        assert!(q.request(2.0, &POLICY, req(), Msg::Got).is_empty(), "fresh");
        q.invalidate();
        assert_eq!(q.request(3.0, &POLICY, req(), Msg::Got).len(), 1, "invalidated: refetch");
    }

    #[test]
    fn non_200_and_parse_failures_are_failures() {
        let mut q: Query<String> = Query::new();
        let no_retry = QueryPolicy { retries: 0, ..POLICY };
        let _ = q.request(0.0, &no_retry, req(), Msg::Got);
        let _ = q.resolve(
            1.0,
            &no_retry,
            HttpResult::Response { status: 503, body: vec![] },
            parse_ok,
            Msg::Retry,
        );
        assert_eq!(q.error(), Some("HTTP 503"));

        let mut q: Query<String> = Query::new();
        let _ = q.request(0.0, &no_retry, req(), Msg::Got);
        fn parse_never(_: &[u8]) -> Result<String, String> {
            Err("bad json".into())
        }
        let _ = q.resolve(1.0, &no_retry, ok("x"), parse_never, Msg::Retry);
        assert_eq!(q.error(), Some("parse: bad json"));
    }
}
