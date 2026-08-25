#!/usr/bin/env bash
# Prove Gloss is usable: tests green, docs match shipped code, undo exists.
set -euo pipefail

root="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$root"

die() {
	printf 'check-usable: %s\n' "$1" >&2
	exit 1
}

docs="$root/applications/gloss/docs"

# Current-path claims that the code does not implement.
if grep -n '| File watching | `notify`' "$docs/02-architecture.md"; then
	die "02-architecture still lists notify as the current watcher"
fi
if grep -n '| Markdown parse | `comrak`' "$docs/02-architecture.md"; then
	die "02-architecture still lists comrak as the current parser"
fi
if grep -n '| Git backing | `gix`' "$docs/02-architecture.md"; then
	die "02-architecture still lists gix as the current checkpoint store"
fi
if grep -n 'oxc/Rolldown\|oxlint' "$docs/05-delivery.md"; then
	die "05-delivery still names oxc/Rolldown as the current toolchain"
fi
if grep -n '| Swift | SwiftPM' "$docs/05-delivery.md"; then
	die "05-delivery still lists SwiftPM as a current toolchain row"
fi

grep -q 'status: superseded' "$docs/adr-003-codemirror.md" \
	|| die "adr-003-codemirror must stay superseded"
grep -q 'Libra-native' "$docs/adr-009-libra-native.md" \
	|| die "adr-009-libra-native is missing"
grep -q 'byte scanner' "$docs/02-architecture.md" \
	|| die "02-architecture must name the shipped byte scanner"

# App undo is part of usable: a save must be reversible from the window.
if ! grep -q 'fn undo' "$root/applications/gloss/src/actions.rs"; then
	die "app has no undo action"
fi
if ! grep -q 'fn undo_' "$root/applications/gloss/tests/integration.rs" \
	&& ! grep -q 'fn undo' "$root/applications/gloss/tests/integration.rs"; then
	die "no integration test covers undo"
fi
if ! grep -q 'CreateNote' "$root/applications/gloss/crates/gloss-cli/tests/cli.rs" \
	|| ! grep -q 'ReplaceSpan' "$root/applications/gloss/crates/gloss-cli/tests/cli.rs"; then
	die "CLI test must apply a multi-op patch (CreateNote + ReplaceSpan)"
fi

cargo test -p gloss-core -p gloss-cli -p gloss

printf 'check-usable: ok\n'
