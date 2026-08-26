# Gloss — agent brief

Product docs: `docs/`. Start with `docs/11-coherence.md` (what and why),
then `docs/00-gloss.md`. Gloss work vs Libra: `docs/10-libra.md`. M1 and
M2 do not wait on Libra. Do not add a catalog node or Effect "for Gloss"
while MCP is unusable. Native menus are §19.3 (`menu:` / `context_menu:`
on `app!`); do not paint a menubar. The vault switcher is the
`gloss/workspace` Kv JSON blob.

If you are editing the Gloss **UI** (`.lx`, gallery-facing chrome, the
editor shell), read `docs/catalog-handoff.md`. The shell already uses
`<Toolbar>`, `<Search>`, `<Tabs>`/`<Tab>`, `<StatusBar>`, and `<Toast>`.
Do not reimplement them with `ListItem` + ghost buttons.
