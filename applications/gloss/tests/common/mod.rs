use gloss::{Gloss, STORE_KEY};
use libra_core::{FolderPick, KvResult, ProcessResult, ProcessStatus};
use libra_test::{by, fx, Harness};

pub fn harness() -> Harness<Gloss> {
    harness_at(960, 640)
}

pub fn harness_at(w: u32, h: u32) -> Harness<Gloss> {
    let mut h = Harness::<Gloss>::with_viewport(gloss::VIEW, w, h);
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
    h.click(by::test_id("browse-vault"));
    let pick = h.expect_effect(fx::pick_folder());
    h.resolve_folder_pick(pick, FolderPick::Picked("/tmp/gloss-fixture".into()));
    let kv = h.expect_effect(fx::kv_set(STORE_KEY));
    h.resolve_kv_set(kv, KvResult::Ok);
    let list = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(list, ok_stdout(listing));
}

pub fn resolve_rules(h: &mut Harness<Gloss>, body: &str) {
    if h
        .pending_effects()
        .any(|e| matches!(e, libra_core::Effect::Process { .. }))
    {
        let rules = h.expect_effect(fx::process_program("gloss"));
        h.resolve_process(rules, ok_stdout(body));
    }
}

pub fn open_first_note(h: &mut Harness<Gloss>, listing: &str, body: &str) {
    open_and_list(h, listing);
    if listing.trim().is_empty() {
        resolve_rules(h, "");
        return;
    }
    let cat = h.expect_effect(fx::process_program("gloss"));
    h.resolve_process(cat, ok_stdout(body));
    resolve_rules(h, "");
}

#[allow(dead_code)]
pub fn enter_edit(h: &mut Harness<Gloss>) {
    if h.state().show_preview {
        h.click(by::test_id("toggle-preview"));
    }
}

pub fn assert_no_overflow(h: &Harness<Gloss>) {
    let laid = h.laid();
    for (i, item) in laid.items.iter().enumerate() {
        if item.clip.is_some() {
            continue;
        }
        let r = item.rect;
        if r.w <= 0.0 && r.h <= 0.0 {
            continue;
        }
        let past_right = r.x + r.w > laid.width + 1.0;
        let past_bottom = r.y + r.h > laid.height + 1.0;
        let past_left = r.x < -1.0;
        let past_top = r.y < -1.0;
        if past_right || past_bottom || past_left || past_top {
            panic!(
                "item {i} {:?} rect {:?} extends past viewport {}x{}",
                item.semantics.test_id, r, laid.width, laid.height
            );
        }
    }
}
