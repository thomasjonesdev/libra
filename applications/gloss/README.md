# Gloss

Point it at your existing Markdown vault. No import. The product is a
bundle that composes (graph, review, capture, stale docs, MCP), not an
Obsidian skin. Writes go through one patch layer.

What and why: `docs/11-coherence.md`. Architecture:
`architecture/sections/27.2-gloss.md`.

## Run

```sh
cargo run -p gloss
```

Click Browse to pick a vault folder. New notes land in `10-Inbox/`. Edits
autosave through `gloss apply`. Undo reverts the last journaled run.

Rail: **C** quick-open, **D** daily note, **G** links/backlinks drawer.
Function map: `docs/09-app-functions.md`.

CLI, same binary or `gloss-cli`:

```sh
cargo run -p gloss -- list --root path/to/vault
cargo run -p gloss-cli -- apply --root path/to/vault --patch patch.json
```

## Test

```sh
cargo test -p gloss-core
cargo test -p gloss-cli
cargo test -p gloss
```

## What this is not yet

Embeddings, MCP, review surface, Tab, audio. The index crate has notes and
resolved links. Soft wrap and `Effect::File*` are Libra work, not missing
rail buttons. See `docs/04-roadmap.md` and `docs/10-libra.md`.
