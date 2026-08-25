use std::fs;
use std::path::PathBuf;
use std::process::Command;

use tempfile::tempdir;

fn gloss() -> Command {
    let exe = std::env::current_exe().expect("test exe");
    let debug_dir = exe
        .parent()
        .and_then(|p| p.parent())
        .expect("target debug dir");
    Command::new(debug_dir.join("gloss-cli"))
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vault-fixtures/mini")
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let to = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), to).unwrap();
        }
    }
}

#[test]
fn list_and_apply_undo_round_trip() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("mini");
    copy_dir(&fixture(), &vault);
    let journal = tmp.path().join("journal.json");

    let list = gloss()
        .args(["list", "--root"])
        .arg(&vault)
        .output()
        .unwrap();
    assert!(
        list.status.success(),
        "{}",
        String::from_utf8_lossy(&list.stderr)
    );
    let listed = String::from_utf8(list.stdout).unwrap();
    assert!(listed.contains("Atlas.md"));
    assert!(listed.contains("10-Inbox/Welcome.md"));

    let hash = gloss()
        .args(["hash", "--root"])
        .arg(&vault)
        .args(["--path", "Atlas.md"])
        .output()
        .unwrap();
    assert!(hash.status.success());
    let expect = String::from_utf8(hash.stdout).unwrap().trim().to_string();

    let patch_path = tmp.path().join("patch.json");
    let patch = format!(
        r#"{{
          "id": "p1",
          "run": "r1",
          "ops": [{{
            "CreateNote": {{
              "root": 1,
              "path": "Inbox/from-cli.md",
              "content": "hello from cli"
            }}
          }}],
          "rationale": "cli test",
          "provenance": null
        }}"#
    );
    let _ = expect;
    fs::write(&patch_path, patch).unwrap();

    let apply = gloss()
        .args(["apply", "--root"])
        .arg(&vault)
        .arg("--patch")
        .arg(&patch_path)
        .arg("--journal")
        .arg(&journal)
        .output()
        .unwrap();
    assert!(
        apply.status.success(),
        "{}",
        String::from_utf8_lossy(&apply.stderr)
    );
    assert_eq!(
        fs::read_to_string(vault.join("Inbox/from-cli.md")).unwrap(),
        "hello from cli"
    );

    let undo = gloss()
        .args(["undo", "--root"])
        .arg(&vault)
        .arg("--journal")
        .arg(&journal)
        .output()
        .unwrap();
    assert!(
        undo.status.success(),
        "{}",
        String::from_utf8_lossy(&undo.stderr)
    );
    assert!(!vault.join("Inbox/from-cli.md").exists());
}
