# Libra Markup — editor extension

TextMate grammar for `.lx` view files. Day-1 editor tooling from
[architecture §18.6](../architecture/sections/18.6-editor-tooling.md): syntax
highlighting in Cursor / VS Code (and any editor that consumes TextMate
grammars — Zed, Helix, etc.).

## What you get

- Language id `lx` for `*.lx`
- Catalog elements, control tags (`for` / `if` / `else`), and sources
  (`Frames` / `Interval` / `Stream`) colored distinctly
- Attribute forms from the closed grammar: flags, `"strings"`, `{numbers}`,
  `{bindings}`, handler calls `onPress={action(item.id)}`
- Text interpolations `{count}` / `{items.len()}`
- `//` line comments (same as `libra-markup`)
- Bracket matching + indent rules

Not yet: completions, diagnostics, go-to-def — that is `libra lsp` (v0.2 in §18.6).

## Install for Cursor (this machine)

From the repo root:

```sh
./editors/vscode/install.sh
```

Then reload the window (`Developer: Reload Window`). Open any `examples/*/views/*.lx`
and the language mode should show **Libra Markup**.

The script symlinks `editors/vscode` into `~/.cursor/extensions/libra.libra-lx-0.1.0`
(falls back to `~/.vscode/extensions` if Cursor's folder is missing).

## Manual / VS Code

```sh
# Cursor
ln -sfn "$(pwd)/editors/vscode" ~/.cursor/extensions/libra.libra-lx-0.1.0

# VS Code
ln -sfn "$(pwd)/editors/vscode" ~/.vscode/extensions/libra.libra-lx-0.1.0
```

Reload the editor after linking.

## Development Host (no install)

Open `editors/vscode` in Cursor/VS Code and run **Extensions: Development Host**
(F5). A new window loads the grammar against the workspace.

## Other editors

The grammar lives at [`vscode/syntaxes/lx.tmLanguage.json`](vscode/syntaxes/lx.tmLanguage.json).
Point Zed / Helix / Sublime at that file (or copy it) under language id `lx` /
scope `source.lx`.

## Keeping it honest

When the catalog grows (new element name in both engines + a conformance
fixture), add the PascalCase name to the `entity.name.tag.lx` alternation in
the TextMate grammar so unknown tags stay visually distinct from known ones.
