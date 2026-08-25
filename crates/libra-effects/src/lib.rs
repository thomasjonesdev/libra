//! libra-effects — the effects runtime, the only I/O actor: a worker pool
//! runs http/kv/random/process and dedicated threads run timers; results
//! return as actions through the runtime's `Sink`. Cross-effect ordering is
//! deliberately unguaranteed — state must tolerate reordering.
//!
//! Architecture: ARCHITECTURE.md §12.3 (execution model), §12.6 (process effect).

use libra_core::{
    CommandSpec, Effect, HttpRequest, HttpResult, KvResult, ProcessId, ProcessResult,
    ProcessStatus, TimerId,
};
use libra_data::SeriesStore;
use libra_kv::Kv;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
    cancelled_timers: Arc<Mutex<HashSet<TimerId>>>,
    process_allowlist: HashSet<String>,
    cancelled_processes: Arc<Mutex<HashSet<ProcessId>>>,
    running_processes: Arc<Mutex<HashMap<ProcessId, Arc<AtomicBool>>>>,
    next_process_id: Arc<AtomicU64>,
}

const WORKERS: usize = 4;

const FILTERED_ENV_KEYS: &[&str] = &[
    "HOME", "USERPROFILE", "PATH", "LANG", "LC_ALL", "LC_CTYPE", "SYSTEMROOT", "TEMP", "TMP",
];

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
            cancelled_timers: Arc::new(Mutex::new(HashSet::new())),
            process_allowlist: HashSet::new(),
            cancelled_processes: Arc::new(Mutex::new(HashSet::new())),
            running_processes: Arc::new(Mutex::new(HashMap::new())),
            next_process_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Programs the worker pool may spawn (§12.6). Empty allowlist rejects all.
    pub fn with_process_allowlist(mut self, programs: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.process_allowlist = programs.into_iter().map(Into::into).collect();
        self
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
                let cancelled = Arc::clone(&self.cancelled_timers);
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
                self.cancelled_timers.lock().expect("cancel set").insert(id);
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
            Effect::Process { id, command, on_done } => {
                let process_id = id.unwrap_or_else(|| self.alloc_process_id());
                let sink = Arc::clone(&self.sink);
                let allowlist = self.process_allowlist.clone();
                let cancelled = Arc::clone(&self.cancelled_processes);
                let running = Arc::clone(&self.running_processes);
                self.spawn(move || {
                    let result = do_process(&command, &allowlist, process_id, &running);
                    if cancelled.lock().expect("cancel set").remove(&process_id) {
                        return;
                    }
                    running.lock().expect("running set").remove(&process_id);
                    sink(on_done(result));
                });
            }
            Effect::CancelProcess { id } => {
                self.cancelled_processes.lock().expect("cancel set").insert(id);
                if let Some(cancel) = self.running_processes.lock().expect("running set").remove(&id) {
                    cancel.store(true, Ordering::Relaxed);
                }
            }
        }
    }

    fn alloc_process_id(&self) -> ProcessId {
        ProcessId(self.next_process_id.fetch_add(1, Ordering::Relaxed))
    }

    fn spawn(&self, job: impl FnOnce() + Send + 'static) {
        self.work_tx.send(Box::new(job)).expect("worker pool alive");
    }
}

fn unix_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn apply_filtered_env(cmd: &mut Command, extra: &[(String, String)]) {
    cmd.env_clear();
    for key in FILTERED_ENV_KEYS {
        if *key == "PATH" {
            cmd.env("PATH", augmented_path());
            continue;
        }
        if let Ok(val) = std::env::var(key) {
            cmd.env(key, val);
        }
    }
    for (k, v) in extra {
        cmd.env(k, v);
    }
}

/// Packaged GUI apps inherit a minimal PATH from the OS launcher — not the
/// login-shell PATH where Homebrew, snap, and MSI installs land. Prepend the
/// usual CLI locations on each platform so allowlisted programs resolve (§12.6).
fn augmented_path() -> String {
    let mut dirs = Vec::new();
    platform_cli_dirs(&mut dirs);
    user_cli_dirs(&mut dirs);
    if let Ok(existing) = std::env::var("PATH") {
        for part in existing.split(path_separator()) {
            push_unique(&mut dirs, part.to_string());
        }
    }
    dirs.join(path_separator())
}

fn platform_cli_dirs(dirs: &mut Vec<String>) {
    #[cfg(target_os = "macos")]
    macos_cli_dirs(dirs);
    #[cfg(target_os = "linux")]
    linux_cli_dirs(dirs);
    #[cfg(windows)]
    windows_cli_dirs(dirs);
}

#[cfg(target_os = "macos")]
fn macos_cli_dirs(dirs: &mut Vec<String>) {
    push_unique(dirs, "/opt/homebrew/bin".into());
    push_unique(dirs, "/usr/local/bin".into());
    if let Ok(parts) = read_etc_paths() {
        for part in parts {
            push_unique(dirs, part);
        }
    }
}

#[cfg(target_os = "linux")]
fn linux_cli_dirs(dirs: &mut Vec<String>) {
    for p in [
        "/usr/local/bin",
        "/usr/local/sbin",
        "/usr/bin",
        "/bin",
        "/snap/bin",
        "/home/linuxbrew/.linuxbrew/bin",
    ] {
        push_unique(dirs, p.into());
    }
    if let Ok(parts) = read_etc_environment_path() {
        for part in parts {
            push_unique(dirs, part);
        }
    }
}

#[cfg(windows)]
fn windows_cli_dirs(dirs: &mut Vec<String>) {
    const AWS: &str = r"Amazon\AWSCLIV2";
    if let Ok(pf) = std::env::var("ProgramFiles") {
        push_unique(dirs, format!(r"{pf}\{AWS}"));
    }
    if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
        push_unique(dirs, format!(r"{pf86}\{AWS}"));
    }
    if let Ok(pd) = std::env::var("ProgramData") {
        push_unique(dirs, format!(r"{pd}\chocolatey\bin"));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        push_unique(dirs, format!(r"{local}\Programs"));
        push_unique(dirs, format!(r"{local}\Microsoft\WindowsApps"));
    }
    // Fallbacks when the process env omits ProgramFiles (some GUI launchers).
    push_unique(dirs, r"C:\Program Files\Amazon\AWSCLIV2".into());
    push_unique(dirs, r"C:\Program Files (x86)\Amazon\AWSCLIV2".into());
    push_unique(dirs, r"C:\ProgramData\chocolatey\bin".into());
}

fn user_cli_dirs(dirs: &mut Vec<String>) {
    let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) else {
        return;
    };
    let sep = std::path::MAIN_SEPARATOR;
    push_unique(dirs, format!("{home}{sep}.local{sep}bin"));
    #[cfg(windows)]
    {
        push_unique(dirs, format!("{home}\\scoop\\shims"));
    }
    #[cfg(not(windows))]
    {
        push_unique(dirs, format!("{home}{sep}.cargo{sep}bin"));
    }
}

fn push_unique(dirs: &mut Vec<String>, dir: String) {
    if !dir.is_empty() && !dirs.iter().any(|d| d == &dir) {
        dirs.push(dir);
    }
}

fn path_separator() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

#[cfg(target_os = "macos")]
fn read_etc_paths() -> Result<Vec<String>, std::io::Error> {
    use std::io::{BufRead, BufReader};

    let mut dirs = Vec::new();
    if let Ok(paths) = std::fs::read_to_string("/etc/paths") {
        for line in paths.lines() {
            let line = line.trim();
            if !line.is_empty() {
                push_unique(&mut dirs, line.to_string());
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir("/etc/paths.d") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(file) = std::fs::File::open(&path) {
                    for line in BufReader::new(file).lines().map_while(Result::ok) {
                        let line = line.trim();
                        if !line.is_empty() {
                            push_unique(&mut dirs, line.to_string());
                        }
                    }
                }
            }
        }
    }
    Ok(dirs)
}

/// Login PATH from `/etc/environment` (Debian/Ubuntu/GNOME etc.).
#[cfg(target_os = "linux")]
fn read_etc_environment_path() -> Result<Vec<String>, std::io::Error> {
    let text = std::fs::read_to_string("/etc/environment")?;
    let mut dirs = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("PATH=") else {
            continue;
        };
        let rest = rest.trim().trim_matches('"').trim_matches('\'');
        for part in rest.split(':') {
            let part = part.trim();
            if !part.is_empty() {
                push_unique(&mut dirs, part.to_string());
            }
        }
    }
    Ok(dirs)
}

fn do_process(
    command: &CommandSpec,
    allowlist: &HashSet<String>,
    process_id: ProcessId,
    running: &Arc<Mutex<HashMap<ProcessId, Arc<AtomicBool>>>>,
) -> ProcessResult {
    let finished_at = unix_ms_now();
    if !allowlist.contains(&command.program) {
        return ProcessResult {
            status: ProcessStatus::SpawnFailed(format!(
                "program '{}' is not on the runtime allowlist",
                command.program
            )),
            stdout: vec![],
            stderr: vec![],
            finished_at_unix_ms: finished_at,
        };
    }
    if command.program.contains('/') || command.program.contains('\\') {
        return ProcessResult {
            status: ProcessStatus::SpawnFailed(
                "program name must not contain path separators".into(),
            ),
            stdout: vec![],
            stderr: vec![],
            finished_at_unix_ms: finished_at,
        };
    }

    let mut cmd = Command::new(&command.program);
    cmd.args(&command.args);
    apply_filtered_env(&mut cmd, &command.env);
    if let Some(cwd) = &command.cwd {
        cmd.current_dir(cwd);
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            return ProcessResult {
                status: ProcessStatus::SpawnFailed(e.to_string()),
                stdout: vec![],
                stderr: vec![],
                finished_at_unix_ms: finished_at,
            };
        }
    };

    let cancel_flag = Arc::new(AtomicBool::new(false));
    running.lock().expect("running set").insert(process_id, Arc::clone(&cancel_flag));

    let timeout = command.timeout_ms.map(Duration::from_millis);
    let start = std::time::Instant::now();
    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return ProcessResult {
                status: ProcessStatus::Signalled(9),
                stdout: vec![],
                stderr: vec![],
                finished_at_unix_ms: unix_ms_now(),
            };
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(mut out) = child.stdout.take() {
                    let _ = out.read_to_end(&mut stdout);
                }
                if let Some(mut err) = child.stderr.take() {
                    let _ = err.read_to_end(&mut stderr);
                }
                #[cfg(unix)]
                let proc_status = {
                    use std::os::unix::process::ExitStatusExt;
                    if let Some(code) = status.code() {
                        ProcessStatus::Exited(code)
                    } else if let Some(sig) = status.signal() {
                        ProcessStatus::Signalled(sig)
                    } else {
                        ProcessStatus::Exited(-1)
                    }
                };
                #[cfg(windows)]
                let proc_status = ProcessStatus::Exited(status.code().unwrap_or(-1));
                return ProcessResult {
                    status: proc_status,
                    stdout,
                    stderr,
                    finished_at_unix_ms: unix_ms_now(),
                };
            }
            Ok(None) => {
                if let Some(limit) = timeout {
                    if start.elapsed() >= limit {
                        let _ = child.kill();
                        let _ = child.wait();
                        let mut stderr = Vec::new();
                        if let Some(mut err) = child.stderr.take() {
                            let _ = err.read_to_end(&mut stderr);
                        }
                        return ProcessResult {
                            status: ProcessStatus::TimedOut,
                            stdout: vec![],
                            stderr,
                            finished_at_unix_ms: unix_ms_now(),
                        };
                    }
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return ProcessResult {
                    status: ProcessStatus::SpawnFailed(e.to_string()),
                    stdout: vec![],
                    stderr: vec![],
                    finished_at_unix_ms: unix_ms_now(),
                };
            }
        }
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
    use libra_core::CommandSpec;
    use std::sync::mpsc::channel;

    #[derive(Debug, Clone, PartialEq)]
    enum Action {
        Loaded(Option<Vec<u8>>),
        Saved(KvResult),
        Tick,
        Entropy(Vec<u8>),
        Fetched(HttpResult),
        Ran(ProcessResult),
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

    fn runtime_with_allowlist(
        programs: &[&str],
    ) -> (Effects<Action>, mpsc::Receiver<Action>, tempfile::TempDir) {
        let (fx, rx, dir) = runtime();
        let fx = fx.with_process_allowlist(programs.iter().map(|s| (*s).to_string()));
        (fx, rx, dir)
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

    #[test]
    fn augmented_path_prepends_platform_cli_dirs() {
        let path = augmented_path();
        let parts: Vec<&str> = path.split(path_separator()).collect();
        assert!(!parts.is_empty(), "PATH must not be empty: {path}");
        #[cfg(target_os = "macos")]
        {
            let brew = parts
                .iter()
                .position(|&p| p == "/opt/homebrew/bin")
                .expect("homebrew bin");
            let usr = parts
                .iter()
                .position(|&p| p == "/usr/bin")
                .expect("/usr/bin from /etc/paths or parent");
            assert!(brew < usr, "Homebrew must precede system PATH: {path}");
        }
        #[cfg(target_os = "linux")]
        {
            let local = parts
                .iter()
                .position(|&p| p == "/usr/local/bin")
                .expect("/usr/local/bin");
            let usr = parts.iter().position(|&p| p == "/usr/bin").expect("/usr/bin");
            assert!(local < usr, "user/local bin must precede system PATH: {path}");
        }
        #[cfg(windows)]
        {
            assert!(
                parts.iter().any(|p| p.contains("AWSCLIV2")),
                "AWS CLI install dir must be on PATH: {path}"
            );
        }
    }

    #[test]
    fn process_rejects_unlisted_program() {
        let (fx, rx, _dir) = runtime_with_allowlist(&["echo"]);
        fx.execute(Effect::process(
            CommandSpec::new("true"),
            Action::Ran,
        ));
        match recv(&rx) {
            Action::Ran(r) => assert!(matches!(r.status, ProcessStatus::SpawnFailed(_))),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn process_runs_allowlisted_program() {
        let (fx, rx, _dir) = runtime_with_allowlist(&["echo"]);
        fx.execute(Effect::process(
            CommandSpec::new("echo").arg("Mind the gap."),
            Action::Ran,
        ));
        match recv(&rx) {
            Action::Ran(r) => {
                assert!(matches!(r.status, ProcessStatus::Exited(0)));
                assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), "Mind the gap.");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn cancelled_process_never_delivers() {
        let (fx, rx, _dir) = runtime_with_allowlist(&["sleep"]);
        let id = ProcessId(42);
        fx.execute(Effect::process_id(
            id,
            CommandSpec::new("sleep").arg("5").timeout_ms(10_000),
            Action::Ran,
        ));
        std::thread::sleep(Duration::from_millis(100));
        fx.execute(Effect::CancelProcess { id });
        assert!(rx.recv_timeout(Duration::from_millis(500)).is_err());
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
