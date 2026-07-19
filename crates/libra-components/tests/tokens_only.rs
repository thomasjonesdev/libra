//! PLAN-TOKENS T3 / PLAN-COMPONENTS C3 enforcement: catalog components style
//! through **named theme tokens only** — never a raw color literal. With paint
//! localized one-file-per-component (PLAN-COMPONENTS C1), this is a trivial
//! source scan. Beauty comes from the token table, so a new component can be
//! re-themed without editing catalog code (§11).
//!
//! Allowed: `theme.<token>` and the `Color::TRANSPARENT` sentinel. Forbidden:
//! `Color::rgb(` / `Color::rgba(` and inline `0x..` hex color literals.

use std::fs;
use std::path::Path;

#[test]
fn catalog_paint_uses_tokens_not_raw_colors() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/catalog");
    let mut offenders = Vec::new();

    for entry in fs::read_dir(&dir).expect("catalog dir exists") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let src = fs::read_to_string(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        for (i, line) in src.lines().enumerate() {
            let code = line.split("//").next().unwrap_or(""); // ignore comments
            if code.contains("Color::rgb(") || code.contains("Color::rgba(") {
                offenders.push(format!("{name}:{}  raw color literal — use a theme token", i + 1));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "catalog components must style through theme tokens only (PLAN-TOKENS T3):\n  {}",
        offenders.join("\n  ")
    );
}
