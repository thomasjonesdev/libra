//! Spawn-backed streams (PLAN-STREAM-DELIVERY, §12.3, §12.6).
//!
//! One thread owns each child. Lines are buffered until a `<Stream>`
//! handler mounts, then replayed in order. A full buffer with no
//! handler becomes one `Dropped(n)`. Unmount or `CloseStream` kills
//! the child. Output is never logged.

use libra_core::{Capture, CommandSpec, ProcessStatus, StreamClose, StreamEvent};
use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const RING: usize = 256;
const BATCH: usize = 64;
const MAX_LINE: usize = 64 * 1024;

pub(crate) struct Streams {
    slots: Mutex<HashMap<String, Slot>>,
    wake: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    next_gen: AtomicU64,
}

struct Slot {
    gen: u64,
    live: bool,
    pending: VecDeque<StreamEvent>,
    dropped: u32,
    cancel: Arc<AtomicBool>,
}

pub(crate) struct StreamDrain {
    pub events: Vec<(String, StreamEvent)>,
    pub more: bool,
}

impl Streams {
    pub(crate) fn new() -> Self {
        Streams {
            slots: Mutex::new(HashMap::new()),
            wake: Mutex::new(None),
            next_gen: AtomicU64::new(1),
        }
    }

    pub(crate) fn set_wake(&self, wake: Arc<dyn Fn() + Send + Sync>) {
        *self.wake.lock().expect("stream wake") = Some(wake);
    }

    pub(crate) fn close(&self, id: &str) {
        if let Some(slot) = self.slots.lock().expect("streams").get(id) {
            slot.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Mounted ids replay their buffer (and any live lines). Ids that
    /// disappeared are killed and dropped — the handler is gone, so
    /// `Closed` has nowhere to go.
    pub(crate) fn sync(&self, mounted: &[String]) -> StreamDrain {
        let mut slots = self.slots.lock().expect("streams");
        let gone: Vec<String> = slots
            .keys()
            .filter(|id| !mounted.iter().any(|m| m == *id))
            .cloned()
            .collect();
        for id in gone {
            if let Some(slot) = slots.remove(&id) {
                slot.cancel.store(true, Ordering::Relaxed);
            }
        }
        let mut events = Vec::new();
        let mut more = false;
        for id in mounted {
            let Some(slot) = slots.get_mut(id) else { continue };
            slot.live = true;
            let queued = take_pending(slot);
            let room = BATCH.saturating_sub(events.len());
            if queued.len() > room {
                more = true;
                let mut queued = queued;
                let rest = queued.split_off(room);
                for ev in rest.into_iter().rev() {
                    slot.pending.push_front(ev);
                }
                events.extend(queued.into_iter().map(|ev| (id.clone(), ev)));
                break;
            }
            events.extend(queued.into_iter().map(|ev| (id.clone(), ev)));
            if events.len() >= BATCH {
                more = slots.values().any(|s| !s.pending.is_empty());
                break;
            }
        }
        if !more {
            more = slots.values().any(|s| !s.pending.is_empty());
        }
        drop(slots);
        if more {
            self.wake();
        }
        StreamDrain { events, more }
    }

    fn wake(&self) {
        if let Some(wake) = self.wake.lock().expect("stream wake").as_ref() {
            wake();
        }
    }
}

/// Shared registry. `Effects` holds `Arc<Streams>` and the thread does too.
impl Streams {
    pub(crate) fn spawn_shared(
        self: &Arc<Self>,
        id: String,
        command: CommandSpec,
        capture: Capture,
        allowlist: &std::collections::HashSet<String>,
    ) {
        let allowlist = allowlist.clone();
        let gen = self.next_gen.fetch_add(1, Ordering::Relaxed);
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut slots = self.slots.lock().expect("streams");
            if let Some(old) = slots.remove(&id) {
                old.cancel.store(true, Ordering::Relaxed);
            }
            slots.insert(
                id.clone(),
                Slot {
                    gen,
                    live: false,
                    pending: VecDeque::new(),
                    dropped: 0,
                    cancel: Arc::clone(&cancel),
                },
            );
        }
        let streams = Arc::clone(self);
        std::thread::spawn(move || {
            run_child(&streams, &id, gen, &cancel, &command, capture, &allowlist);
        });
    }
}

fn take_pending(slot: &mut Slot) -> Vec<StreamEvent> {
    let mut queued: Vec<StreamEvent> = slot.pending.drain(..).collect();
    if slot.dropped > 0 {
        let ev = StreamEvent::Dropped(slot.dropped);
        slot.dropped = 0;
        if matches!(queued.last(), Some(StreamEvent::Closed(_))) {
            let closed = queued.pop().unwrap();
            queued.push(ev);
            queued.push(closed);
        } else {
            queued.push(ev);
        }
    }
    queued
}

fn enqueue(slot: &mut Slot, event: StreamEvent) {
    let terminal = matches!(event, StreamEvent::Closed(_));
    if !slot.live && !terminal && content_len(slot) >= RING {
        slot.dropped = slot.dropped.saturating_add(1);
        return;
    }
    slot.pending.push_back(event);
}

fn content_len(slot: &Slot) -> usize {
    slot.pending
        .iter()
        .filter(|e| !matches!(e, StreamEvent::Closed(_)))
        .count()
}

fn push_event(streams: &Streams, id: &str, gen: u64, event: StreamEvent) {
    let wake = {
        let mut slots = streams.slots.lock().expect("streams");
        let Some(slot) = slots.get_mut(id) else { return };
        if slot.gen != gen {
            return;
        }
        let wake = slot.live;
        enqueue(slot, event);
        wake
    };
    if wake {
        streams.wake();
    }
}

fn run_child(
    streams: &Streams,
    id: &str,
    gen: u64,
    cancel: &AtomicBool,
    command: &CommandSpec,
    capture: Capture,
    allowlist: &std::collections::HashSet<String>,
) {
    let fail = |msg: String| {
        push_event(
            streams,
            id,
            gen,
            StreamEvent::Closed(StreamClose::Process(ProcessStatus::SpawnFailed(msg))),
        );
    };
    if !allowlist.contains(&command.program) {
        fail(format!(
            "program '{}' is not on the runtime allowlist",
            command.program
        ));
        return;
    }
    if command.program.contains('/') || command.program.contains('\\') {
        fail("program name must not contain path separators".into());
        return;
    }

    let mut cmd = Command::new(&command.program);
    cmd.args(&command.args);
    crate::apply_filtered_env(&mut cmd, &command.env);
    if let Some(cwd) = &command.cwd {
        cmd.current_dir(cwd);
    }
    match capture {
        Capture::Stdout => {
            cmd.stdout(Stdio::piped()).stderr(Stdio::null());
        }
        Capture::Stderr => {
            cmd.stdout(Stdio::null()).stderr(Stdio::piped());
        }
        Capture::Both => {
            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        }
    }
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            fail(e.to_string());
            return;
        }
    };

    let (tx, rx) = mpsc::channel();
    if let Some(out) = child.stdout.take() {
        let tx = tx.clone();
        std::thread::spawn(move || read_pipe(out, false, tx));
    }
    if let Some(err) = child.stderr.take() {
        let tx = tx.clone();
        std::thread::spawn(move || read_pipe(err, true, tx));
    }
    drop(tx);

    let start = Instant::now();
    let timeout = command.timeout_ms.map(Duration::from_millis);
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            while let Ok(ev) = rx.try_recv() {
                push_event(streams, id, gen, ev);
            }
            push_event(streams, id, gen, StreamEvent::Closed(StreamClose::Cancelled));
            return;
        }
        match rx.recv_timeout(Duration::from_millis(25)) {
            Ok(ev) => push_event(streams, id, gen, ev),
            Err(RecvTimeoutError::Timeout) => {
                if let Some(limit) = timeout {
                    if start.elapsed() >= limit {
                        let _ = child.kill();
                        let _ = child.wait();
                        while let Ok(ev) = rx.try_recv() {
                            push_event(streams, id, gen, ev);
                        }
                        push_event(
                            streams,
                            id,
                            gen,
                            StreamEvent::Closed(StreamClose::Process(ProcessStatus::TimedOut)),
                        );
                        return;
                    }
                }
                match child.try_wait() {
                    Ok(Some(status)) => {
                        drain_rest(streams, id, gen, &rx);
                        push_event(
                            streams,
                            id,
                            gen,
                            StreamEvent::Closed(StreamClose::Process(status_of(status))),
                        );
                        return;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        fail(e.to_string());
                        return;
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                let status = match child.wait() {
                    Ok(status) => status_of(status),
                    Err(e) => ProcessStatus::SpawnFailed(e.to_string()),
                };
                push_event(
                    streams,
                    id,
                    gen,
                    StreamEvent::Closed(StreamClose::Process(status)),
                );
                return;
            }
        }
    }
}

fn drain_rest(streams: &Streams, id: &str, gen: u64, rx: &Receiver<StreamEvent>) {
    let deadline = Instant::now() + Duration::from_millis(200);
    loop {
        match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(ev) => push_event(streams, id, gen, ev),
            Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) if Instant::now() >= deadline => return,
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

fn read_pipe(mut pipe: impl Read, stderr: bool, tx: Sender<StreamEvent>) {
    let mut buf = [0u8; 4096];
    let mut line = Vec::new();
    loop {
        match pipe.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                for &b in &buf[..n] {
                    if b == b'\n' {
                        if line.last() == Some(&b'\r') {
                            line.pop();
                        }
                        let text = String::from_utf8_lossy(&line).into_owned();
                        line.clear();
                        if tx.send(StreamEvent::Line { text, stderr }).is_err() {
                            return;
                        }
                    } else if line.len() >= MAX_LINE {
                        let text = String::from_utf8_lossy(&line).into_owned();
                        line.clear();
                        if tx.send(StreamEvent::Line { text, stderr }).is_err() {
                            return;
                        }
                        line.push(b);
                    } else {
                        line.push(b);
                    }
                }
            }
            Err(_) => break,
        }
    }
    if !line.is_empty() {
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        let text = String::from_utf8_lossy(&line).into_owned();
        let _ = tx.send(StreamEvent::Line { text, stderr });
    }
}

#[cfg(unix)]
fn status_of(status: std::process::ExitStatus) -> ProcessStatus {
    use std::os::unix::process::ExitStatusExt;
    if let Some(code) = status.code() {
        ProcessStatus::Exited(code)
    } else if let Some(sig) = status.signal() {
        ProcessStatus::Signalled(sig)
    } else {
        ProcessStatus::Exited(-1)
    }
}

#[cfg(not(unix))]
fn status_of(status: std::process::ExitStatus) -> ProcessStatus {
    ProcessStatus::Exited(status.code().unwrap_or(-1))
}

#[cfg(test)]
impl Streams {
    fn test_open(self: &Arc<Self>, id: &str) {
        let gen = self.next_gen.fetch_add(1, Ordering::Relaxed);
        self.slots.lock().expect("streams").insert(
            id.to_string(),
            Slot {
                gen,
                live: false,
                pending: VecDeque::new(),
                dropped: 0,
                cancel: Arc::new(AtomicBool::new(false)),
            },
        );
    }

    fn test_push(self: &Arc<Self>, id: &str, event: StreamEvent) {
        let gen = self.slots.lock().expect("streams").get(id).unwrap().gen;
        push_event(self, id, gen, event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn line(text: &str) -> StreamEvent {
        StreamEvent::Line {
            text: text.into(),
            stderr: false,
        }
    }

    #[test]
    fn buffer_replays_in_order_when_the_handler_mounts() {
        let streams = Arc::new(Streams::new());
        streams.test_open("log");
        streams.test_push("log", line("one"));
        streams.test_push("log", line("two"));
        streams.test_push(
            "log",
            StreamEvent::Closed(StreamClose::Process(ProcessStatus::Exited(0))),
        );
        let drain = streams.sync(&["log".into()]);
        assert_eq!(
            drain.events,
            vec![
                ("log".into(), line("one")),
                ("log".into(), line("two")),
                (
                    "log".into(),
                    StreamEvent::Closed(StreamClose::Process(ProcessStatus::Exited(0)))
                ),
            ]
        );
    }

    #[test]
    fn overflow_before_mount_is_one_dropped_event() {
        let streams = Arc::new(Streams::new());
        streams.test_open("log");
        for i in 0..300 {
            streams.test_push("log", line(&i.to_string()));
        }
        streams.test_push(
            "log",
            StreamEvent::Closed(StreamClose::Process(ProcessStatus::Exited(0))),
        );
        let mut got = Vec::new();
        loop {
            let drain = streams.sync(&["log".into()]);
            if drain.events.is_empty() {
                break;
            }
            got.extend(drain.events);
        }
        assert_eq!(got.len(), 258); // 256 kept + Dropped + Closed
        assert!(matches!(got[255].1, StreamEvent::Line { .. }));
        assert_eq!(got[256].1, StreamEvent::Dropped(44));
        assert!(matches!(
            got[257].1,
            StreamEvent::Closed(StreamClose::Process(ProcessStatus::Exited(0)))
        ));
    }

    #[test]
    fn spawn_echo_delivers_the_line_then_closed() {
        let streams = Arc::new(Streams::new());
        let mut allow = HashSet::new();
        allow.insert("echo".into());
        streams.spawn_shared(
            "log".into(),
            CommandSpec::new("echo").arg("hello"),
            Capture::Stdout,
            &allow,
        );
        let mut got = Vec::new();
        for _ in 0..50 {
            let drain = streams.sync(&["log".into()]);
            got.extend(drain.events);
            if got.iter().any(|(_, ev)| matches!(ev, StreamEvent::Closed(_))) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            got.iter().any(|(_, ev)| matches!(
                ev,
                StreamEvent::Line { text, stderr: false } if text == "hello"
            )),
            "events: {got:?}"
        );
        assert!(matches!(
            got.last().map(|(_, ev)| ev),
            Some(StreamEvent::Closed(StreamClose::Process(ProcessStatus::Exited(0))))
        ));
    }

    #[test]
    fn close_kills_a_running_child() {
        let streams = Arc::new(Streams::new());
        let mut allow = HashSet::new();
        allow.insert("sleep".into());
        streams.spawn_shared(
            "log".into(),
            CommandSpec::new("sleep").arg("30"),
            Capture::Stdout,
            &allow,
        );
        std::thread::sleep(Duration::from_millis(50));
        let _ = streams.sync(&["log".into()]);
        streams.close("log");
        let mut closed = false;
        for _ in 0..40 {
            let drain = streams.sync(&["log".into()]);
            if drain
                .events
                .iter()
                .any(|(_, ev)| matches!(ev, StreamEvent::Closed(StreamClose::Cancelled)))
            {
                closed = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(closed, "CloseStream must end the stream with Cancelled");
    }
}
