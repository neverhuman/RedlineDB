#!/usr/bin/env bash
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

jain_ci_scratch_create "$ROOT_DIR" redline-web-governed-jankurai \
  || fail "unable to create a custody-safe in-repository scratch directory"
tmp="$JAIN_CI_SCRATCH_PATH"
cleanup() {
  local rc=$?
  trap - EXIT
  jain_ci_scratch_remove || exit 1
  exit "$rc"
}
trap cleanup EXIT

fixture="$tmp/jankurai"
cat >"$fixture" <<'FIXTURE'
#!/usr/bin/env bash
[[ "${1:-}" == "--version" ]] || exit 64
printf 'jankurai fixture-1\n'
FIXTURE
chmod 0755 "$fixture"
fixture_digest="$(jain_sha256 "$fixture")"

jain_verify_governed_jankurai "$fixture" "jankurai fixture-1" "$fixture_digest"
if jain_verify_governed_jankurai "$tmp/missing" "jankurai fixture-1" "$fixture_digest"; then
  printf 'missing governed Jankurai was accepted\n' >&2
  exit 1
fi
ln -s "$fixture" "$tmp/linked-jankurai"
if jain_verify_governed_jankurai "$tmp/linked-jankurai" "jankurai fixture-1" "$fixture_digest"; then
  printf 'symlinked governed Jankurai was accepted\n' >&2
  exit 1
fi
if jain_verify_governed_jankurai "$fixture" "jankurai fixture-1" \
  "0000000000000000000000000000000000000000000000000000000000000000"; then
  printf 'wrong governed Jankurai digest was accepted\n' >&2
  exit 1
fi
if jain_verify_governed_jankurai "$fixture" "jankurai fixture-2" "$fixture_digest"; then
  printf 'wrong governed Jankurai version was accepted\n' >&2
  exit 1
fi

mkdir "$tmp/hostile-bin"
cat >"$tmp/hostile-bin/jankurai" <<'HOSTILE'
#!/usr/bin/env bash
printf 'hostile PATH jankurai\n'
exit 0
HOSTILE
chmod 0755 "$tmp/hostile-bin/jankurai"
resolved="$(PATH="$tmp/hostile-bin:$PATH" jankurai_bin)"
[[ "$resolved" == "$JAIN_GOVERNED_JANKURAI_BIN" ]] || {
  printf 'hostile PATH selected Jankurai: %s\n' "$resolved" >&2
  exit 1
}

printf 'governed Jankurai hostile identity tests passed\n'
