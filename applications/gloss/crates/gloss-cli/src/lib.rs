use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use gloss_core::{
    apply_op, apply_patch, find_backlinks, hash_bytes, list_notes, Journal, NoteId, Patch, RelPath,
    VaultPath, Workspace,
};
use gloss_index::Index;

pub fn is_cli_command(cmd: &str) -> bool {
    matches!(
        cmd,
        "list"
            | "cat"
            | "hash"
            | "apply"
            | "undo"
            | "backlinks"
            | "index"
            | "neighbors"
            | "suggest-links"
    )
}

pub fn run() -> ExitCode {
    run_args(env::args().skip(1))
}

pub fn run_args(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(cmd) = args.next() else {
        eprintln!("{}", usage());
        return ExitCode::from(2);
    };
    match cmd.as_str() {
        "list" => cmd_list(&mut args),
        "cat" => cmd_cat(&mut args),
        "hash" => cmd_hash(&mut args),
        "apply" => cmd_apply(&mut args),
        "undo" => cmd_undo(&mut args),
        "backlinks" => cmd_backlinks(&mut args),
        "index" => cmd_index(&mut args),
        "neighbors" => cmd_neighbors(&mut args),
        "suggest-links" => cmd_suggest_links(&mut args),
        _ => {
            eprintln!("{}", usage());
            ExitCode::from(2)
        }
    }
}

fn usage() -> &'static str {
    "usage:\n  gloss list --root DIR\n  gloss cat --root DIR --path REL\n  gloss hash --root DIR --path REL\n  gloss backlinks --root DIR --path REL\n  gloss index --root DIR --db FILE\n  gloss neighbors --root DIR --path REL --db FILE\n  gloss suggest-links --root DIR --db FILE\n  gloss apply --root DIR --patch FILE [--journal FILE]\n  gloss undo --root DIR --journal FILE"
}

fn cmd_index(args: &mut impl Iterator<Item = String>) -> ExitCode {
    let (root, db) = match flags_root_db(args) {
        Ok(v) => v,
        Err(e) => return bad(&e),
    };
    let mut ws = Workspace::new();
    if let Err(e) = ws.add_root(&root, root_name(&root)) {
        return bad(&e.to_string());
    }
    let mut index = match Index::open(&db) {
        Ok(i) => i,
        Err(e) => return bad(&e.to_string()),
    };
    match index.refresh(&ws, 0) {
        Ok(stats) => {
            println!(
                "scanned {} updated {} unchanged {} deleted {}",
                stats.scanned, stats.updated, stats.unchanged, stats.deleted
            );
            ExitCode::SUCCESS
        }
        Err(e) => bad(&e.to_string()),
    }
}

fn cmd_neighbors(args: &mut impl Iterator<Item = String>) -> ExitCode {
    let items: Vec<String> = args.collect();
    let mut root = None;
    let mut rel = None;
    let mut db = None;
    let mut i = 0;
    while i < items.len() {
        match items[i].as_str() {
            "--root" => {
                i += 1;
                root = items.get(i).cloned();
            }
            "--path" => {
                i += 1;
                rel = items.get(i).cloned();
            }
            "--db" => {
                i += 1;
                db = items.get(i).cloned();
            }
            _ => return bad("unknown flag"),
        }
        i += 1;
    }
    let Some(root) = root else {
        return bad("missing --root");
    };
    let Some(rel) = rel else {
        return bad("missing --path");
    };
    let Some(db) = db else {
        return bad("missing --db");
    };
    let mut ws = Workspace::new();
    let id = match ws.add_root(&root, root_name(&root)) {
        Ok(id) => id,
        Err(e) => return bad(&e.to_string()),
    };
    let path = match RelPath::parse(&rel) {
        Ok(p) => p,
        Err(e) => return bad(&e.to_string()),
    };
    let index = match Index::open(&db) {
        Ok(i) => i,
        Err(e) => return bad(&e.to_string()),
    };
    match index.neighbors(&NoteId::new(id, path)) {
        Ok(n) => {
            for dest in n.outgoing {
                println!("outgoing {}", dest.path);
            }
            for src in n.incoming {
                println!("incoming {}", src.path);
            }
            ExitCode::SUCCESS
        }
        Err(e) => bad(&e.to_string()),
    }
}

fn cmd_suggest_links(args: &mut impl Iterator<Item = String>) -> ExitCode {
    let (root, db) = match flags_root_db(args) {
        Ok(v) => v,
        Err(e) => return bad(&e),
    };
    let mut ws = Workspace::new();
    if let Err(e) = ws.add_root(&root, root_name(&root)) {
        return bad(&e.to_string());
    }
    let mut index = match Index::open(&db) {
        Ok(i) => i,
        Err(e) => return bad(&e.to_string()),
    };
    if let Err(e) = index.refresh(&ws, 0) {
        return bad(&e.to_string());
    }
    match index.suggest_links(&ws) {
        Ok(hits) => {
            for hit in hits {
                println!(
                    "{} -> {}  {} @{}",
                    hit.src.path, hit.dst.path, hit.matched, hit.byte_start
                );
            }
            ExitCode::SUCCESS
        }
        Err(e) => bad(&e.to_string()),
    }
}

fn flags_root_db(args: &mut impl Iterator<Item = String>) -> Result<(String, String), String> {
    let items: Vec<String> = args.collect();
    let mut root = None;
    let mut db = None;
    let mut i = 0;
    while i < items.len() {
        match items[i].as_str() {
            "--root" => {
                i += 1;
                root = items.get(i).cloned();
            }
            "--db" => {
                i += 1;
                db = items.get(i).cloned();
            }
            _ => return Err("unknown flag".into()),
        }
        i += 1;
    }
    Ok((
        root.ok_or_else(|| "missing --root".to_string())?,
        db.ok_or_else(|| "missing --db".to_string())?,
    ))
}

fn cmd_backlinks(args: &mut impl Iterator<Item = String>) -> ExitCode {
    let (ws, path) = match open_file(args) {
        Ok(v) => v,
        Err(e) => return bad(&e),
    };
    match find_backlinks(&ws, path.root, &path.rel) {
        Ok(notes) => {
            for n in notes {
                println!("{n}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => bad(&e.to_string()),
    }
}

fn cmd_list(args: &mut impl Iterator<Item = String>) -> ExitCode {
    let root = match flag(args, "--root") {
        Some(p) => p,
        None => return bad("missing --root"),
    };
    let mut ws = Workspace::new();
    let id = match ws.add_root(&root, root_name(&root)) {
        Ok(id) => id,
        Err(e) => return bad(&e.to_string()),
    };
    match list_notes(ws.root(id).unwrap()) {
        Ok(notes) => {
            for n in notes {
                println!("{}", n);
            }
            ExitCode::SUCCESS
        }
        Err(e) => bad(&e.to_string()),
    }
}

fn cmd_cat(args: &mut impl Iterator<Item = String>) -> ExitCode {
    let (ws, path) = match open_file(args) {
        Ok(v) => v,
        Err(e) => return bad(&e),
    };
    match ws.read(&path) {
        Ok(bytes) => {
            let _ = std::io::Write::write_all(&mut std::io::stdout(), &bytes);
            ExitCode::SUCCESS
        }
        Err(e) => bad(&e.to_string()),
    }
}

fn cmd_hash(args: &mut impl Iterator<Item = String>) -> ExitCode {
    let (ws, path) = match open_file(args) {
        Ok(v) => v,
        Err(e) => return bad(&e),
    };
    match ws.read(&path) {
        Ok(bytes) => {
            println!("{}", hash_bytes(&bytes).to_hex());
            ExitCode::SUCCESS
        }
        Err(e) => bad(&e.to_string()),
    }
}

fn cmd_apply(args: &mut impl Iterator<Item = String>) -> ExitCode {
    let mut root = None;
    let mut patch_file = None;
    let mut patch_json = None;
    let mut journal_file = None;
    let items: Vec<String> = args.collect();
    let mut i = 0;
    while i < items.len() {
        match items[i].as_str() {
            "--root" => {
                i += 1;
                root = items.get(i).cloned();
            }
            "--patch" => {
                i += 1;
                patch_file = items.get(i).cloned();
            }
            "--patch-json" => {
                i += 1;
                patch_json = items.get(i).cloned();
            }
            "--journal" => {
                i += 1;
                journal_file = items.get(i).cloned();
            }
            _ => return bad("unknown apply flag"),
        }
        i += 1;
    }
    let Some(root) = root else {
        return bad("missing --root");
    };
    let raw = match (patch_file, patch_json) {
        (Some(path), None) => match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => return bad(&e.to_string()),
        },
        (None, Some(json)) => json,
        _ => return bad("need exactly one of --patch or --patch-json"),
    };
    let patch: Patch = match serde_json::from_str(&raw) {
        Ok(p) => p,
        Err(e) => return bad(&e.to_string()),
    };
    let mut ws = Workspace::new();
    if let Err(e) = ws.add_root(&root, root_name(&root)) {
        return bad(&e.to_string());
    }
    match apply_patch(&ws, &patch) {
        Ok(inverse) => {
            if let Some(jf) = journal_file {
                let mut journal = load_journal(&jf);
                journal.record(patch.run, patch.id, inverse);
                if let Err(e) = save_journal(&jf, &journal) {
                    return bad(&e);
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => bad(&e.to_string()),
    }
}

fn cmd_undo(args: &mut impl Iterator<Item = String>) -> ExitCode {
    let mut root = None;
    let mut journal_file = None;
    let items: Vec<String> = args.collect();
    let mut i = 0;
    while i < items.len() {
        match items[i].as_str() {
            "--root" => {
                i += 1;
                root = items.get(i).cloned();
            }
            "--journal" => {
                i += 1;
                journal_file = items.get(i).cloned();
            }
            _ => return bad("unknown undo flag"),
        }
        i += 1;
    }
    let Some(root) = root else {
        return bad("missing --root");
    };
    let Some(journal_file) = journal_file else {
        return bad("missing --journal");
    };
    let mut journal = load_journal(&journal_file);
    let Some(run) = journal.last_run() else {
        return bad("journal is empty");
    };
    let Some(ops) = journal.inverse_for(run).map(|o| o.to_vec()) else {
        return bad("missing inverse");
    };
    let mut ws = Workspace::new();
    if let Err(e) = ws.add_root(&root, root_name(&root)) {
        return bad(&e.to_string());
    }
    for op in ops {
        if let Err(e) = apply_op(&ws, &op) {
            return bad(&e.to_string());
        }
    }
    journal.runs.pop();
    if let Err(e) = save_journal(&journal_file, &journal) {
        return bad(&e);
    }
    ExitCode::SUCCESS
}

fn open_file(args: &mut impl Iterator<Item = String>) -> Result<(Workspace, VaultPath), String> {
    let items: Vec<String> = args.collect();
    let mut root = None;
    let mut rel = None;
    let mut i = 0;
    while i < items.len() {
        match items[i].as_str() {
            "--root" => {
                i += 1;
                root = items.get(i).cloned();
            }
            "--path" => {
                i += 1;
                rel = items.get(i).cloned();
            }
            _ => return Err("unknown flag".into()),
        }
        i += 1;
    }
    let root = root.ok_or_else(|| "missing --root".to_string())?;
    let rel = rel.ok_or_else(|| "missing --path".to_string())?;
    let mut ws = Workspace::new();
    let id = ws
        .add_root(&root, root_name(&root))
        .map_err(|e| e.to_string())?;
    let path = RelPath::parse(&rel).map_err(|e| e.to_string())?;
    Ok((
        ws,
        VaultPath {
            root: id,
            rel: path,
        },
    ))
}

fn load_journal(path: &str) -> Journal {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_journal(path: &str, journal: &Journal) -> Result<(), String> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(journal).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| e.to_string())
}

fn flag(args: &mut impl Iterator<Item = String>, name: &str) -> Option<String> {
    let items: Vec<String> = args.collect();
    let mut i = 0;
    while i < items.len() {
        if items[i] == name {
            return items.get(i + 1).cloned();
        }
        i += 1;
    }
    None
}

fn root_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "vault".into())
}

fn bad(msg: &str) -> ExitCode {
    eprintln!("gloss: {msg}");
    ExitCode::from(1)
}
