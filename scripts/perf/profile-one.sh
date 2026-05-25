#!/usr/bin/env bash
# Profile a single redline-testing case in isolation.
#
# Usage:
#   scripts/perf/profile-one.sh <case-name> [--iters N]
#   scripts/perf/profile-one.sh SCALAR_STRING_028 --iters 500
#
# Extracts the case stdin/args/db from the corpus snapshot, rebuilds
# `redlinedb` with debug symbols + frame pointers (so perf can resolve
# call graphs), then replays the case N times under perf record (and
# optionally cargo-flamegraph + heaptrack). Artifacts land in
# target/perf/profile/<case>/.
#
# Required:  redline-testing corpus snapshot at target/perf/corpus-snapshot.json
#             (run scripts/perf/build-case-lists.sh first)
# Optional:  perf, cargo-flamegraph, heaptrack

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
source "$(dirname "$0")/lib.sh"

case_name="${1:?usage: profile-one.sh <case-name> [--iters N]}"
shift || true
iters=200
while [ $# -gt 0 ]; do
  case "$1" in
    --iters) iters="$2"; shift 2 ;;
    *) printf 'unknown flag: %s\n' "$1" >&2; exit 2 ;;
  esac
done

snapshot="$PERF_ROOT/corpus-snapshot.json"
if [ ! -f "$snapshot" ]; then
  printf 'corpus snapshot missing: %s\n' "$snapshot" >&2
  printf '       run scripts/perf/build-case-lists.sh first\n' >&2
  exit 2
fi

out_dir="$PERF_ROOT/profile/$case_name"
mkdir -p "$out_dir"

python3 - "$snapshot" "$case_name" "$out_dir" <<'PYEOF'
import json
import pathlib
import sys

snap_path, name, outdir = sys.argv[1], sys.argv[2], pathlib.Path(sys.argv[3])
cases = json.load(open(snap_path))
case = next((c for c in cases if c.get("name") == name), None)
if not case:
    print(f"no such case in snapshot: {name}", file=sys.stderr)
    sys.exit(2)
(outdir / "stdin.txt").write_text(case.get("stdin", "") or "")
(outdir / "args.txt").write_text(" ".join(case.get("args", [])))
(outdir / "db.txt").write_text(case.get("db", ":memory:"))
(outdir / "case.json").write_text(json.dumps(case, indent=2))
print(
    f"case {case['id']:05d} {case['name']}: "
    f"stdin_bytes={len(case.get('stdin') or '')} "
    f"args={case.get('args')!r} db={case.get('db')!r}"
)
PYEOF

printf '==> building redlinedb with debug-symbols-in-release-native\n'
RUSTFLAGS="${RUSTFLAGS:-} -C target-cpu=native -C force-frame-pointers=yes -C debuginfo=2" \
  cargo build --profile release-native -p redlinedb-cli --bin redlinedb --locked

target_bin="target/release-native/redlinedb"
if [ ! -x "$target_bin" ]; then
  printf '==> release-native build failed; falling back to release\n'
  cargo build --release -p redlinedb-cli --bin redlinedb --locked
  target_bin="target/release/redlinedb"
fi

stdin_file="$out_dir/stdin.txt"
db_arg="$(cat "$out_dir/db.txt")"
mapfile -t case_args < <(tr ' ' '\n' < "$out_dir/args.txt" | grep -v '^$' || true)

run_driver() {
  local i=0
  while [ $i -lt "$iters" ]; do
    if [ "${#case_args[@]}" -gt 0 ]; then
      "$target_bin" "${case_args[@]}" < "$stdin_file" > /dev/null 2>&1 || true
    else
      "$target_bin" "$db_arg" < "$stdin_file" > /dev/null 2>&1 || true
    fi
    i=$((i + 1))
  done
}

# Export so the bash -c subshells below see the driver state.
export target_bin stdin_file db_arg iters
case_args_export=("${case_args[@]}")

printf '==> baseline timing (%s iters)\n' "$iters"
time run_driver

if command -v perf >/dev/null 2>&1; then
  printf '==> perf record (%s iters)\n' "$iters"
  perf record -F 997 --call-graph dwarf -o "$out_dir/perf.data" -- \
    bash -c '
      i=0
      while [ "$i" -lt "$iters" ]; do
        if [ '"${#case_args[@]}"' -gt 0 ]; then
          "$target_bin" '"${case_args[@]@Q}"' < "$stdin_file" > /dev/null 2>&1 || true
        else
          "$target_bin" "$db_arg" < "$stdin_file" > /dev/null 2>&1 || true
        fi
        i=$((i+1))
      done
    '
  perf report --stdio --no-children -i "$out_dir/perf.data" 2>/dev/null | head -120 > "$out_dir/perf-report.txt"
  printf '    wrote %s/perf.data and perf-report.txt\n' "$out_dir"
else
  printf 'WARN: perf not installed; skipping perf record\n'
fi

if command -v cargo-flamegraph >/dev/null 2>&1 || command -v flamegraph >/dev/null 2>&1; then
  printf '==> flamegraph\n'
  flamegraph_cmd="$(command -v flamegraph || command -v cargo-flamegraph)"
  CARGO_PROFILE_RELEASE_DEBUG=true "$flamegraph_cmd" -o "$out_dir/flame.svg" -- \
    bash -c '
      i=0
      while [ "$i" -lt "$iters" ]; do
        if [ '"${#case_args[@]}"' -gt 0 ]; then
          "$target_bin" '"${case_args[@]@Q}"' < "$stdin_file" > /dev/null 2>&1 || true
        else
          "$target_bin" "$db_arg" < "$stdin_file" > /dev/null 2>&1 || true
        fi
        i=$((i+1))
      done
    ' || printf 'WARN: flamegraph failed\n'
fi

if command -v heaptrack >/dev/null 2>&1; then
  printf '==> heaptrack (single iteration, allocator profile)\n'
  if [ "${#case_args[@]}" -gt 0 ]; then
    heaptrack -o "$out_dir/heap" -- "$target_bin" "${case_args[@]}" < "$stdin_file" > /dev/null 2>&1 || true
  else
    heaptrack -o "$out_dir/heap" -- "$target_bin" "$db_arg" < "$stdin_file" > /dev/null 2>&1 || true
  fi
else
  printf 'WARN: heaptrack not installed; skipping allocator profile\n'
fi

printf 'done. artifacts in %s/\n' "$out_dir"
ls -lh "$out_dir/" 2>/dev/null
