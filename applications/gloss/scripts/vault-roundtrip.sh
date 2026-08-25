#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$root"

die() {
	printf 'vault-roundtrip: %s\n' "$1" >&2
	exit 1
}

cargo build -p gloss -q
target="$(cargo metadata --format-version 1 --no-deps --offline | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
gloss="$target/debug/gloss"
[[ -x "$gloss" ]] || die "missing $gloss"

fixture="$root/applications/gloss/vault-fixtures/mini"
[[ -d "$fixture" ]] || die "missing mini fixture"

work="$(mktemp -d "${TMPDIR:-/tmp}/gloss-roundtrip.XXXXXX")"
trap 'rm -rf "$work"' EXIT
vault="$work/mini"
mkdir -p "$vault"
cp -R "$fixture/." "$vault/"
journal="$vault/.gloss/journal.json"

list="$("$gloss" list --root "$vault")"
printf '%s\n' "$list" | grep -q 'Atlas.md' || die "list missed Atlas.md"

cp "$vault/Atlas.md" "$work/Atlas.md.orig"
expect="$("$gloss" hash --root "$vault" --path Atlas.md | tr -d '[:space:]')"
[[ ${#expect} -eq 64 ]] || die "hash did not print 64 hex chars"

patch="$(python3 -c '
import json, sys
old = open(sys.argv[1], "rb").read()
patch = {
    "id": "edit",
    "run": "edit",
    "ops": [{
        "ReplaceSpan": {
            "note": {"root": 1, "path": "Atlas.md"},
            "start": 0,
            "end": len(old),
            "text": "from-json\n",
            "expect_hash": sys.argv[2],
        }
    }],
    "rationale": "editor save",
    "provenance": None,
}
print(json.dumps(patch))
' "$vault/Atlas.md" "$expect")"

"$gloss" apply --root "$vault" --patch-json "$patch" --journal "$journal"
printf 'from-json\n' >"$work/Atlas.md.applied"
cmp -s "$vault/Atlas.md" "$work/Atlas.md.applied" || die "apply --patch-json did not replace Atlas.md"
[[ -f "$journal" ]] || die "apply did not write the journal"

"$gloss" undo --root "$vault" --journal "$journal"
cmp -s "$vault/Atlas.md" "$work/Atlas.md.orig" || die "undo did not restore Atlas.md bytes"

if [[ -d "$root/architecture" ]]; then
	arch_list="$("$gloss" list --root "$root/architecture")"
	printf '%s\n' "$arch_list" | grep -q 'sections/27.2-gloss.md' || die "architecture list missed 27.2-gloss.md"
fi

printf 'vault-roundtrip: ok\n'
