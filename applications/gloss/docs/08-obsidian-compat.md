---
title: Obsidian compatibility
type: spec
status: draft
created: 2026-08-24
tags: [gloss, architecture]
---

# Obsidian compatibility

Gloss points at existing Obsidian vaults and must be safe to run alongside
Obsidian on the same directory, with both open. This is a baseline constraint on
the patch layer, not a feature — see [[decisions/adr-008-obsidian-compat]].

Three rules, in priority order:

1. **Never corrupt.** A file Gloss doesn't fully understand is opaque and
   untouchable, not a best-effort rewrite.
2. **Never reformat.** Only the bytes inside a patch range change. No prettier
   pass, no YAML re-serialization, no line-ending normalization.
3. **Resolve links exactly as Obsidian does**, or backlinks will silently
   disagree between the two apps and the user will trust neither.

## Syntax that must parse

The byte scanner in `parse.rs` currently handles wikilinks, embeds, `%%`
comments, and a raw YAML split. The rest of this table is the compatibility
target, not the shipped parser.

| Construct | Forms |
| --- | --- |
| Wikilinks | `[[note]]`, `[[note\|alias]]`, `[[note#Heading]]`, `[[note#^blockid]]` |
| Embeds | `![[note]]`, `![[img.png\|300]]`, `![[note#Heading]]` |
| Block IDs | trailing `^block-id` on a line or paragraph |
| Markdown links | `[text](note.md)`, URL-encoded, relative and absolute |
| Tags | `#tag`, nested `#area/sub`, plus `tags:` in frontmatter |
| Frontmatter | YAML — `aliases`, `tags`, `cssclasses`; list *or* scalar forms |
| Callouts | `> [!note]`, `> [!warning] Title`, foldable `> [!tip]-` |
| Comments | `%%inline%%` and block form — excluded from indexing |
| Footnotes | `[^1]` references and definitions |
| Math | `$inline$`, `$$block$$` |
| Dataview | `key:: value` inline fields, and `dataview` code blocks |

`tags:` in frontmatter appears as a YAML list, a comma-separated string, or the
legacy singular `tag:`. All three exist in real vaults. Nested tags are
hierarchical: `#area/sub` implies membership in `#area`.

## Link resolution

Obsidian resolves a bare `[[note]]` against a global filename index, not a path.
Getting this wrong is the fastest way to produce a backlink graph that disagrees
with Obsidian's.

- Read `.obsidian/app.json` for `newLinkFormat` (`shortest` / `relative` /
  `absolute`), `useMarkdownLinks`, and `attachmentFolderPath`. Honor the vault's
  setting when *writing* links.
- Resolution order: exact relative path → exact vault-absolute path → unique
  filename match → nearest match by folder proximity when ambiguous.
- **Case-insensitive matching.** macOS filesystems are case-insensitive by
  default and Obsidian resolves accordingly.
- **Unicode normalization is a real trap.** macOS returns filenames in NFD;
  wikilink text inside files is typically NFC. Normalize to NFC for comparison,
  but write back exactly the bytes that were there. A vault with accented or CJK
  filenames will expose this immediately.
- Unresolved links are first-class data, not errors — they're a signal for the
  retroactive link pass.

## Files that are opaque or off-limits

**Never read into the index, never write, never touch:**

- `.obsidian/` — settings, plugins, workspace state. Read `app.json` for
  resolution config only. Writing here corrupts a live Obsidian session.
- `.trash/`
- `.git/`, and anything matched by the vault's `.gitignore`

**Parse for metadata, never rewrite the body:**

- `*.canvas` — JSON. Extract node file references so canvases appear in the
  graph; treat contents as opaque.
- `*.excalidraw.md` — the compressed drawing payload is unreadable to the patch
  layer. Index the frontmatter, nothing else.
- Templater / Templates syntax (`<% %>`, `{{date}}`) — an unrendered template is
  correct as written. Never "fix" it.
- Any file where the parse round-trip fails byte-identically. Fail closed.

## Write fidelity

Every write must preserve:

- Frontmatter key order, quoting style, indentation, and unknown keys
- Line endings as found, per file
- Trailing newline presence or absence
- Filename unicode normalization
- Indentation style of surrounding lists when inserting

`SetFrontmatter` therefore operates as a surgical edit on the YAML block's byte
range — not parse, mutate, re-serialize. Re-serializing a YAML document reorders
keys and drops comments, which is a silent diff on every note it touches and
will show up as noise in the user's git history.

## Gloss's own footprint in the vault

Minimal and Obsidian-legible:

```
<vault>/.gloss/
  rules.md        agent rules — plain Markdown, committed
  ignore          gitignore syntax
  trash/          soft deletes, 30-day sweep
```

Obsidian ignores dotfolders, so none of this appears in the user's file tree or
graph. The index, journal, and models live outside the vault entirely — see
[[03-data-model#Where things live]].

**Frontmatter Gloss writes goes under a single namespaced key**, so Obsidian's
Properties panel shows one collapsed entry rather than five loose fields:

```yaml
gloss:
  code_refs:
    - repo: ~/src/orbital
      target: crates/telemetry/src/derive.rs#TransformSpec
      last_seen_rev: a3f9c21
  indexed: 2026-08-24T14:02:00Z
```

## Acceptance test

The golden-vault tier in [[05-delivery#Testing strategy]] gets one more case,
and it is a release gate:

> Index a copy of a real vault. Apply a no-op patch to every note. Assert the
> directory is byte-identical to the original, including mtimes on untouched
> files.

If that test can't pass, nothing else in the project matters.

## Deliberately not supported in v1

Reading or honoring community plugin data formats beyond Dataview inline fields.
There are thousands, they have no schema, and chasing them is unbounded. Opaque
is the correct default.
