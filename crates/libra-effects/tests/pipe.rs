//! §12.1 `Effect::Process`: a child's stdout must be drained while it runs.
//!
//! A pipe holds ~64 KB. If the runtime waits for exit before reading, any
//! child that writes more than that blocks in `write` forever and the pair
//! deadlocks until the timeout fires — reporting *no* output for a command
//! that actually succeeded. Gloss hits this whenever it `cat`s a note bigger
//! than the pipe buffer.

use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

fn run(bytes: usize) -> libra_core::ProcessResult {
    let (tx, rx) = mpsc::channel::<libra_core::ProcessResult>();
    let sink: libra_effects::Sink<libra_core::ProcessResult> = Arc::new(move |r| {
        let _ = tx.send(r);
    });
    let dir = tempfile::tempdir().expect("tempdir");
    let kv = libra_kv::Kv::open(&dir.path().join("kv.redb")).expect("kv");
    let fx = libra_effects::Effects::new(sink, Some(kv)).with_process_allowlist(["head"]);

    fx.execute(libra_core::Effect::process(
        libra_core::CommandSpec::new("head")
            .args(["-c", &bytes.to_string(), "/dev/zero"])
            .timeout_ms(20_000),
        |r| r,
    ));
    rx.recv_timeout(Duration::from_secs(30))
        .unwrap_or_else(|e| panic!("{bytes} bytes: no result ({e})"))
}

#[test]
fn stdout_larger_than_the_pipe_buffer_is_delivered_whole() {
    // 8 KB fits in a pipe; 200 KB does not, and is the case that used to hang.
    for bytes in [8_000usize, 40_000, 200_000] {
        let started = std::time::Instant::now();
        let result = run(bytes);
        assert_eq!(
            result.stdout.len(),
            bytes,
            "{bytes} bytes: truncated stdout ({:?})",
            result.status
        );
        assert_eq!(
            result.status,
            libra_core::ProcessStatus::Exited(0),
            "{bytes} bytes"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{bytes} bytes took {:?} — the reader is not draining concurrently",
            started.elapsed()
        );
    }
}
