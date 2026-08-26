---
title: Catalog handoff
type: note
status: adopted
created: 2026-08-25
tags: [gloss, catalog]
---

# Catalog chrome

Adopted on `gloss-usable` and present in `views/gloss.lx`:
`<Toolbar>`, `<Search>`, `<Tabs>`/`<Tab>`, `<StatusBar>`, `<Toast>`,
`<Dialog>`, `<Drawer>`, `<Split>`, `<Icon>`. Chip is later (facet
filters). CommandPalette is later. Dialog + Search is the palette today.

Do not rebuild any of this with `ListItem` plus ghost buttons. If a
control is missing, read [[10-libra]] before adding a catalog node.

## Markup to keep

```lx
<Tabs testId="editor-tabs">
  <for each={tab_rows} as="t" key={t.id}>
    <Tab selected={t.selected} onPress={selectNote(t.id)} onClose={closeTab(t.id)}>{t.title}</Tab>
  </for>
</Tabs>
```

```lx
<Search value={query} placeholder="filter…" onChange={queryChanged} testId="search" />
```

```lx
<Toolbar vertical>
  <Button variant="ghost" icon="plus" onPress={newNote} testId="new-note" />
  <Spacer />
  <Button variant="ghost" icon="folder" onPress={openVaultPicker} testId="vault-picker" />
  <Button variant="ghost" icon="settings" onPress={openSettings} testId="settings" />
</Toolbar>
```

```lx
<StatusBar testId="status-bar">
  <Text size="sm" muted grow wrap testId="status">{status}</Text>
</StatusBar>
```

Toast timeout is an app `Timer` or `<Interval>`, not a clock inside
`<Toast>`. Cite `§7.1` / PLAN-CATALOG the way the catalog modules do.
