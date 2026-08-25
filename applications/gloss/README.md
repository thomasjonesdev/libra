# Gloss

A Libra app for Markdown vaults. The vault core is headless. The editor is a
native Libra window. Writes go through one patch layer.

Docs live in `docs/`. Architecture note: `architecture/sections/27.2-gloss.md`.

## Run

```sh
cargo run -p gloss
```

Paste a vault path and click Open. New notes land in `10-Inbox/`. Edits
autosave through `gloss apply`. Undo reverts the last journaled run.

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

Index, embeddings, MCP, review surface, Tab, audio. Those stay on the
roadmap in `docs/04-roadmap.md`.
