#!/usr/bin/env bash
# Symlink the Libra .lx TextMate extension into Cursor (or VS Code).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
EXT_ID="libra.libra-lx-0.1.0"

if [[ -d "${HOME}/.cursor/extensions" ]]; then
  DEST="${HOME}/.cursor/extensions/${EXT_ID}"
elif [[ -d "${HOME}/.vscode/extensions" ]]; then
  DEST="${HOME}/.vscode/extensions/${EXT_ID}"
else
  mkdir -p "${HOME}/.cursor/extensions"
  DEST="${HOME}/.cursor/extensions/${EXT_ID}"
fi

ln -sfn "${ROOT}" "${DEST}"
echo "Linked ${ROOT}"
echo "    → ${DEST}"
echo "Reload the editor window to pick up Libra Markup (.lx)."
