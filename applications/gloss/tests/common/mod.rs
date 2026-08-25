use gloss::{Gloss, STORE_KEY};
use libra_core::{KvResult, ProcessResult, ProcessStatus};
use libra_test::{by, fx, Harness};

pub fn harness() -> Harness<Gloss> {
    let mut h = Harness::<Gloss>::with_viewport(gloss::VIEW, 960, 640);
    let load = h.expect_effect(fx::kv_get(STORE_KEY));
    h.resolve_kv_get(load, None);
    h
}

pub fn ok_stdout(stdout: &str) -> ProcessResult {
    ProcessResult {
        status: ProcessStatus::Exited(0),
        stdout: stdout.as_bytes().to_vec(),
        stderr: vec![],
        finished_at_unix_ms: 1,
    }
}

pub fn open_and_list(h: &mut Harness<Gloss>, listing: &str) {
    h.type_text(by::test_id("vault-path"), "/tmp/gloss-fixture");
    h.click(by::test_id("open-vault"));
    let kv = h.expect_effect(fx::kv_set(STORE_KEY));
    h.resolve_kv_set(kv, KvResult::Ok);
    let list = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(list, ok_stdout(listing));
}

pub fn open_first_note(h: &mut Harness<Gloss>, listing: &str, body: &str) {
    open_and_list(h, listing);
    let cat = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(cat, ok_stdout(body));
}
