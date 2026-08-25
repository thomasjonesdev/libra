---
title: Catalog handoff
type: note
status: draft
created: 2026-08-25
tags: [gloss, catalog]
---

# Catalog handoff (for the Gloss UI agent)

Libra catalog work on 2026-08-25 shipped the chrome Gloss is currently
faking with `ListItem` + ghost buttons. **Do not reimplement these.**
Wait until the catalog commits are on your branch (`catalog-search`,
`catalog-chip`, `catalog-tabs`, `catalog-toast` — merge onto
`gloss-usable` if they are not there yet), then swap the markup.
Actions and `tab_rows` / `query` stay as they are.

Gallery PNG conflicts on merge: keep the catalog frames (the sidebar
grew). Re-validate your own visual work after.

## 1. Tabs — do this first

The editor strip is `ListItem` + a ghost `x`. That is what `<Tabs>` /
`<Tab>` are for.

**Replace** the inner `Row grow` + `for` of `ListItem`/`Button` with:

```lx
<Tabs testId="editor-tabs">
  <for each={tab_rows} as="t" key={t.id}>
    <Tab selected={t.selected} onPress={selectNote(t.id)} onClose={closeTab(t.id)}>{t.title}</Tab>
  </for>
</Tabs>
```

Keep Back, Fwd, and `+` (`newTab`) **outside** the strip. `onClose` is
the editor name (Chip uses `onRemove`). The × hit target is
`{testId}-close` if you set a per-tab `testId`; without one, close is
still clickable, just not queryable by id.

`selectNote` / `closeTab` / `newTab` / `openInNewTab` do not change.
`onMenu={openInNewTab(n.id)}` on the file-list `ListItem` stays.

`<for>` is legal inside `<Tabs>` (`resolve_children`). Do not wrap the
loop in an extra `Row`.

Existing tests that `dispatch(Action::CloseTab(…))` keep working. A
click-the-× test can wait until a tab has a `testId`.

## 2. Search — list filter

**Replace** the filter `Input` + clear `Button` with:

```lx
<Search value={query} placeholder="filter…" onChange={queryChanged} testId="search" />
```

Omit `onSubmit` — this is live-filter, not a submit box. Keep
`testId="search"` (tests already use it). Clear can stay a ghost button
that sets `query` empty, or drop it and let the user delete.

## 3. Toast — save / undo notice

Status text in the footer can stay. For a write that succeeded, mount:

```lx
<if test={show_toast}>
  <Toast tone="success" title="Saved" onDismiss={dismissToast} testId="toast">{status}</Toast>
</if>
```

You need `show_toast: bool`, `dismissToast` (unit), and set the flag in
the save/undo success path. Auto-hide is an app `Timer` or `<Interval>`,
**not** a clock inside `<Toast>` (Appendix A). Clicks outside the toast
fall through — it is not a modal.

Tone `danger` for a failed write. This is in-app chrome, not
`Effect::Notify` (PLAN-NOTIFY / OS banners).

## 4. Chip — later

`<Chip selected onPress onRemove>` is the filter-pill. Use it when
Gloss grows facet filters. Not blocking M4.

## 5. Do not wait for

| Thing | Why |
|---|---|
| `Image` | Blocked on a deterministic decode path |
| `CommandPalette` | Still planned; the `C` rail button can stay `notYet` |
| OS notifications | PLAN-NOTIFY, not `<Toast>` |
| `Toolbar` / `StatusBar` | Landing next on the catalog branch. Keep `Row`/`Column` until they exist, then the left rail, list header, and footer are the adoption sites |

## 6. Merge hygiene

- Catalog work lives on `catalog-*` branches in the `libra-search` worktree. Do not edit those files in the dirty `gloss-usable` tree except when stash-merging.
- After merge: `LIBRA_VALIDATE=1 cargo test -p gallery --test gallery` only if you changed catalog paint. Gloss snapshots are yours.
- Cite `§7.1` / `PLAN-CATALOG` the same way the catalog modules do.
