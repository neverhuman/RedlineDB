#!/usr/bin/env bash
# Dump the libsqlite3 public symbol surface as bundled with rusqlite 0.37.0
# (libsqlite3-sys 0.35.0). The list is pinned by the bundled sqlite3.h header
# inside libsqlite3-sys's source — every `SQLITE_API` declaration that names a
# `sqlite3_*` identifier is emitted, one symbol per line.
#
# Output: target/proof/sqlite-full-parity/libsqlite3-symbols.txt
#
# Re-runnable: if the output file already exists AND contains the same set we
# would write, the script is a no-op. Otherwise the file is regenerated.
#
# Drop-in parity gate: this is the reference set that
# `crates/ffi/tests/symbol_diff.rs` diffs RedlineDB's `libredlinedb.{dylib,so}`
# exports against.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

out_dir="target/proof/sqlite-full-parity"
out_file="${out_dir}/libsqlite3-symbols.txt"

mkdir -p "$out_dir"

# Locate libsqlite3-sys source via cargo metadata. The bundled `sqlite3.h`
# lives at `<manifest_dir>/sqlite3/sqlite3.h`. Clear RUSTC_WRAPPER so the
# script works under environments that set it to `sccache` (the workspace
# Justfile) without sccache being on PATH.
manifest_path="$(
    RUSTC_WRAPPER= cargo metadata --format-version 1 2>/dev/null \
        | python3 -c "
import json, sys
data = json.load(sys.stdin)
for pkg in data['packages']:
    if pkg['name'] == 'libsqlite3-sys':
        print(pkg['manifest_path'])
        break
"
)"

if [[ -z "$manifest_path" ]]; then
    echo "error: libsqlite3-sys not found in workspace metadata" >&2
    echo "hint: ensure rusqlite is a dev-dependency and run 'cargo metadata' once" >&2
    exit 1
fi

sqlite_header="$(dirname "$manifest_path")/sqlite3/sqlite3.h"

if [[ ! -f "$sqlite_header" ]]; then
    echo "error: bundled sqlite3.h not found at $sqlite_header" >&2
    exit 1
fi

# Parse SQLITE_API declarations and extract the sqlite3_* identifier.
# Approach: strip block + line comments first so commented-out declarations
# (e.g. inside SQLITE_OMIT_* blocks) don't pollute the set.
tmp_out="$(mktemp)"
trap 'rm -f "$tmp_out"' EXIT

python3 - "$sqlite_header" "$tmp_out" << 'PY'
import re
import sys

src_path, out_path = sys.argv[1], sys.argv[2]
with open(src_path) as fh:
    text = fh.read()

# Strip block comments then line comments.
text = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)
text = re.sub(r"//.*?\n", "\n", text)

names = set()

# Function declarations: SQLITE_API <return-type> [stars] NAME(...
for match in re.finditer(
    r"^SQLITE_API\b[^;{]*?\b(sqlite3_[A-Za-z0-9_]+)\s*\(",
    text,
    flags=re.MULTILINE,
):
    names.add(match.group(1))

# Extern variables / arrays: SQLITE_API SQLITE_EXTERN ... NAME[ or ;
for match in re.finditer(
    r"^SQLITE_API\s+SQLITE_EXTERN\b[^;]*?\b(sqlite3_[A-Za-z0-9_]+)\b",
    text,
    flags=re.MULTILINE,
):
    names.add(match.group(1))

with open(out_path, "w") as fh:
    for name in sorted(names):
        fh.write(name + "\n")
PY

if [[ -f "$out_file" ]] && cmp -s "$tmp_out" "$out_file"; then
    echo "libsqlite3 symbol baseline unchanged: $out_file ($(wc -l < "$out_file") symbols)"
    exit 0
fi

mv "$tmp_out" "$out_file"
trap - EXIT
echo "wrote libsqlite3 symbol baseline: $out_file ($(wc -l < "$out_file") symbols)"
