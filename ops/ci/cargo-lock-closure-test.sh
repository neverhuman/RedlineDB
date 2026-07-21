#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/cargo-lock-closure.sh
source "$repo_root/ops/ci/cargo-lock-closure.sh"
mkdir -p "$repo_root/target/test-tmp"
tmp="$(mktemp -d "$repo_root/target/test-tmp/cargo-lock-closure-test.XXXXXX")"
trap 'rm -rf -- "$tmp"' EXIT
chmod 0700 "$tmp"

success="$tmp/success"
cat >"$success" <<'EOF'
#!/usr/bin/env bash
printf 'nested/Cargo.lock\0Cargo.lock\0'
EOF
chmod 0700 "$success"
jain_capture_sorted_nul "$tmp/sorted.locks" "$success"
mapfile -d '' -t sorted <"$tmp/sorted.locks"
[[ "${#sorted[@]}" == 2 \
  && "${sorted[0]}" == Cargo.lock \
  && "${sorted[1]}" == nested/Cargo.lock \
  && "$(stat -c '%u:%a:%h' -- "$tmp/sorted.locks")" \
    == "$(id -u):600:1" ]]

partial="$tmp/partial"
cat >"$partial" <<'EOF'
#!/usr/bin/env bash
printf 'Cargo.lock\0nested/Cargo.lock\0'
exit 7
EOF
chmod 0700 "$partial"
if jain_capture_sorted_nul "$tmp/partial.locks" "$partial"; then
  printf 'partial lock producer was accepted\n' >&2
  exit 1
fi
[[ ! -e "$tmp/partial.locks" \
  && -z "$(find "$tmp" -maxdepth 1 -name '.partial.locks.*' -print -quit)" ]]

printf 'sentinel\n' >"$tmp/existing.locks"
if jain_capture_sorted_nul "$tmp/existing.locks" "$success"; then
  printf 'pre-existing lock output was replaced\n' >&2
  exit 1
fi
[[ "$(cat "$tmp/existing.locks")" == sentinel ]]
if jain_capture_sorted_nul relative.locks "$success"; then
  printf 'relative lock output was accepted\n' >&2
  exit 1
fi

printf '../Cargo.lock\0' >"$tmp/unsafe.locks"
chmod 0600 "$tmp/unsafe.locks"
if jain_cargo_lock_source_record \
  jain-fixture 0123456789012345678901234567890123456789 \
  "$tmp" "$tmp/unsafe.locks" >/dev/null; then
  printf 'escaping Cargo lock path was accepted\n' >&2
  exit 1
fi

fixture="$tmp/fixture"
mkdir -p "$fixture/nested"
printf 'root\n' >"$fixture/Cargo.lock"
printf 'nested\n' >"$fixture/nested/Cargo.lock"
record="$(jain_cargo_lock_source_record \
  jain-fixture 0123456789012345678901234567890123456789 \
  "$fixture" "$tmp/sorted.locks")"
printf '%s\n' "$record" >"$tmp/records.jsonl"
jain_render_cargo_lock_source_closure \
  "$tmp/records.jsonl" "$tmp/closure.json"
jq -e '
  select(.schema_version == "jain.cargo-lock-source-closure/v1")
  | select(.source_count == 1 and .lock_count == 2)
  | select(.sources[0].repository == "jain-fixture")
  | select(.sources[0].lock_count == 2)
  | select(.lock_sha256s == (.lock_sha256s | sort))
' "$tmp/closure.json" >/dev/null

printf 'checked Cargo lock closure contract ok\n'
