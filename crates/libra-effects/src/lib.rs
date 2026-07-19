//! libra-effects — the effects runtime, the only I/O actor: a worker pool
//! runs http/kv/random and dedicated threads run timers; results return as
//! actions through the runtime's `Sink`. Cross-effect ordering is
//! deliberately unguaranteed — state must tolerate reordering.
//!
//! Architecture: ARCHITECTURE.md §12.3 (execution model).

use libra_core::{Effect, HttpRequest, HttpResult, KvResult, TimerId};
use libra_data::SeriesStore;
use libra_kv::Kv;
use std::collections::HashSet;
use std::io::Read;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Where effect-result actions go. `Send + Sync` because workers call it.
pub type Sink<M> = Arc<dyn Fn(M) + Send + Sync>;

type Job = Box<dyn FnOnce() + Send>;

pub struct Effects<M> {
    sink: Sink<M>,
    kv: Option<Arc<Kv>>,
    /// The data plane (§14): IngestOpen/Close manage source threads
    /// writing into this store's rings.
    store: Arc<SeriesStore>,
    work_tx: mpsc::Sender<Job>,
    cancelled: Arc<Mutex<HashSet<TimerId>>>,
}

const WORKERS: usize = 4;

impl<M: Send + 'static> Effects<M> {
    /// `kv`: None disables the Kv effects (they resolve to errors) — used
    /// when an app has no manifest identity to store under.
    pub fn new(sink: Sink<M>, kv: Option<Kv>) -> Self {
        Self::with_store(sink, kv, Arc::new(SeriesStore::new()))
    }

    pub fn with_store(sink: Sink<M>, kv: Option<Kv>, store: Arc<SeriesStore>) -> Self {
        let (work_tx, work_rx) = mpsc::channel::<Job>();
        let work_rx = Arc::new(Mutex::new(work_rx));
        for _ in 0..WORKERS {
            let rx = Arc::clone(&work_rx);
            std::thread::spawn(move || loop {
                let job = {
                    let guard = rx.lock().expect("worker queue lock");
                    guard.recv()
                };
                match job {
                    Ok(job) => job(),
                    Err(_) => break, // runtime dropped; pool winds down
                }
            });
        }
        Effects {
            sink,
            kv: kv.map(Arc::new),
            store,
            work_tx,
            cancelled: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn store(&self) -> &Arc<SeriesStore> {
        &self.store
    }

    pub fn execute_all(&self, effects: Vec<Effect<M>>) {
        for effect in effects {
            self.execute(effect);
        }
    }

    pub fn execute(&self, effect: Effect<M>) {
        match effect {
            Effect::Http { req, on_done } => {
                let sink = Arc::clone(&self.sink);
                self.spawn(move || sink(on_done(do_http(&req))));
            }
            Effect::KvGet { key, on_done } => {
                let (kv, sink) = (self.kv.clone(), Arc::clone(&self.sink));
                self.spawn(move || {
                    let value = match &kv {
                        Some(kv) => kv.get(&key).unwrap_or(None),
                        None => None,
                    };
                    sink(on_done(value));
                });
            }
            Effect::KvSet { key, value, on_done } => {
                let (kv, sink) = (self.kv.clone(), Arc::clone(&self.sink));
                self.spawn(move || {
                    let result = match &kv {
                        Some(kv) => match kv.set(&key, &value) {
                            Ok(()) => KvResult::Ok,
                            Err(e) => KvResult::Err(e),
                        },
                        None => KvResult::Err("no kv store configured".into()),
                    };
                    if let Some(ctor) = on_done {
                        sink(ctor(result));
                    }
                });
            }
            Effect::KvDelete { key } => {
                let kv = self.kv.clone();
                self.spawn(move || {
                    if let Some(kv) = &kv {
                        let _ = kv.delete(&key);
                    }
                });
            }
            Effect::KvScan { prefix, on_done } => {
                let (kv, sink) = (self.kv.clone(), Arc::clone(&self.sink));
                self.spawn(move || {
                    let hits = match &kv {
                        Some(kv) => kv.scan_prefix(&prefix).unwrap_or_default(),
                        None => vec![],
                    };
                    sink(on_done(hits));
                });
            }
            Effect::Timer { id, after_ms, action } => {
                let sink = Arc::clone(&self.sink);
                let cancelled = Arc::clone(&self.cancelled);
                // Dedicated thread: a sleeping timer must not occupy a
                // pool worker.
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(after_ms));
                    if let Some(id) = id {
                        if cancelled.lock().expect("cancel set").remove(&id) {
                            return; // cancelled while sleeping: never delivered
                        }
                    }
                    sink(action);
                });
            }
            Effect::CancelTimer { id } => {
                self.cancelled.lock().expect("cancel set").insert(id);
            }
            Effect::Random { bytes, on_done } => {
                let sink = Arc::clone(&self.sink);
                self.spawn(move || {
                    let mut buf = vec![0u8; bytes];
                    let out = match getrandom::getrandom(&mut buf) {
                        Ok(()) => buf,
                        Err(_) => vec![], // entropy failure: empty, app decides
                    };
                    sink(on_done(out));
                });
            }
            Effect::IngestOpen { source, series } => self.store.ingest_open(series, source),
            Effect::IngestClose { series } => self.store.ingest_close(series),
            // Shell-handled: the windowed shell intercepts SetTheme
            // before effects reach the pool; reaching here means
            // headless — nothing to restyle.
            Effect::SetTheme { .. } => {}
            // Shell-handled likewise (arboard is a UI-thread concern);
            // headless reach-through is a no-op.
            Effect::ClipboardSet { .. } => {}
        }
    }

    fn spawn(&self, job: impl FnOnce() + Send + 'static) {
        self.work_tx.send(Box::new(job)).expect("worker pool alive");
    }
}

fn do_http(req: &HttpRequest) -> HttpResult {
    let mut r = ureq::request(req.method, &req.url);
    for (k, v) in &req.headers {
        r = r.set(k, v);
    }
    let response = match &req.body {
        Some(body) => r.send_bytes(body),
        None => r.call(),
    };
    match response {
        Ok(resp) => read_response(resp),
        // Any HTTP status is an answer, not a failure (§12.4: every
        // branch — timeout, 500 — is just one more fixture).
        Err(ureq::Error::Status(_, resp)) => read_response(resp),
        Err(ureq::Error::Transport(t)) => HttpResult::Failed(t.to_string()),
    }
}

fn read_response(resp: ureq::Response) -> HttpResult {
    let status = resp.status();
    let mut body = Vec::new();
    match resp.into_reader().read_to_end(&mut body) {
        Ok(_) => HttpResult::Response { status, body },
        Err(e) => HttpResult::Failed(format!("reading body: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[derive(Debug, Clone, PartialEq)]
    enum Action {
        Loaded(Option<Vec<u8>>),
        Saved(KvResult),
        Tick,
        Entropy(Vec<u8>),
        Fetched(HttpResult),
    }

    fn runtime() -> (Effects<Action>, mpsc::Receiver<Action>, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let kv = Kv::open(&dir.path().join("kv.redb")).expect("kv");
        let (tx, rx) = channel();
        let sink: Sink<Action> = Arc::new(move |m| {
            let _ = tx.send(m);
        });
        (Effects::new(sink, Some(kv)), rx, dir)
    }

    fn recv(rx: &mpsc::Receiver<Action>) -> Action {
        rx.recv_timeout(Duration::from_secs(5)).expect("effect result arrives")
    }

    #[test]
    fn kv_set_then_get_roundtrips_through_workers() {
        let (fx, rx, _dir) = runtime();
        fx.execute(Effect::kv_set_acked("k", b"v".to_vec(), Action::Saved));
        assert_eq!(recv(&rx), Action::Saved(KvResult::Ok));
        fx.execute(Effect::kv_get("k", Action::Loaded));
        assert_eq!(recv(&rx), Action::Loaded(Some(b"v".to_vec())));
    }

    #[test]
    fn timer_fires_and_cancelled_timer_never_delivers() {
        let (fx, rx, _dir) = runtime();
        let id = TimerId(7);
        fx.execute(Effect::Timer { id: Some(id), after_ms: 200, action: Action::Tick });
        fx.execute(Effect::CancelTimer { id });
        fx.execute(Effect::Timer { id: None, after_ms: 30, action: Action::Tick });
        // The un-cancelled timer fires…
        assert_eq!(recv(&rx), Action::Tick);
        // …and the cancelled one never does (wait past its deadline).
        assert!(rx.recv_timeout(Duration::from_millis(400)).is_err());
    }

    #[test]
    fn random_yields_requested_bytes() {
        let (fx, rx, _dir) = runtime();
        fx.execute(Effect::Random { bytes: 16, on_done: Action::Entropy });
        match recv(&rx) {
            Action::Entropy(b) => assert_eq!(b.len(), 16),
            other => panic!("unexpected: {other:?}"),
        }
    }

    /// Real HTTP over a real socket — no mocks (§15.4's spirit): a tiny
    /// in-test TCP server serves one canned response.
    #[test]
    fn http_round_trips_against_a_real_socket() {
        use std::io::Write;
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let body = b"Mind the gap.";
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(body);
        });

        let (fx, rx, _dir) = runtime();
        fx.execute(Effect::http(HttpRequest::get(format!("http://{addr}/zen")), Action::Fetched));
        assert_eq!(
            recv(&rx),
            Action::Fetched(HttpResult::Response { status: 200, body: b"Mind the gap.".to_vec() })
        );
    }
}
