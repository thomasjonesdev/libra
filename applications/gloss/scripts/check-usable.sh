#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$root"

die() {
	printf 'check-usable: %s\n' "$1" >&2
	exit 1
}

docs="$root/applications/gloss/docs"
grep -q 'status: superseded' "$docs/adr-003-codemirror.md" \
	|| die "adr-003-codemirror must stay superseded"
grep -q 'Libra-native' "$docs/adr-009-libra-native.md" \
	|| die "adr-009-libra-native is missing"

cargo test -p gloss-core -p gloss-cli -p gloss
"$root/applications/gloss/scripts/vault-roundtrip.sh"

printf 'check-usable: ok\n'
