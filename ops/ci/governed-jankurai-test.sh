#!/usr/bin/env bash
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

redline_container="$(realpath -e -- "$ROOT_DIR/..")"
external_root="$(mktemp -d /tmp/redline-web-governed-jankurai.XXXXXX)"
scratch_created=0
cleanup() {
  local rc=$? cleanup_rc=0
  trap - EXIT
  if (( scratch_created == 1 )); then
    jain_ci_scratch_remove || cleanup_rc=1
    exec {JAIN_CI_SCRATCH_PATH_FD}<&-
    exec {JAIN_CI_SCRATCH_PARENT_FD}<&-
    exec {JAIN_CI_SCRATCH_TARGET_FD}<&-
    exec {JAIN_CI_SCRATCH_ROOT_FD}<&-
    if (( cleanup_rc == 0 )); then
      rmdir -- "$JAIN_CI_SCRATCH_PATH" "$JAIN_CI_SCRATCH_PARENT" \
        "$JAIN_CI_SCRATCH_TARGET" "$external_root" || cleanup_rc=1
    fi
  else
    rmdir -- "$external_root" || cleanup_rc=1
  fi
  if (( rc == 0 && cleanup_rc != 0 )); then
    rc=1
  fi
  exit "$rc"
}
trap cleanup EXIT

external_root="$(realpath -e -- "$external_root")"
[[ -d "$external_root" && ! -L "$external_root" \
  && "$(stat -Lc '%u' -- "$external_root")" -eq "$EUID" \
  && "$(stat -Lc '%a' -- "$external_root")" == 700 ]] \
  || fail "external hostile-fixture root is not a private physical directory"
case "$external_root/" in
  "$redline_container/"*)
    fail "hostile fixture root must remain outside the canonical Redline container"
    ;;
esac

jain_ci_scratch_create "$external_root" redline-web-governed-jankurai \
  || fail "unable to create a custody-safe external scratch directory"
scratch_created=1
tmp="$JAIN_CI_SCRATCH_PATH"

case "$(realpath -e -- "$tmp")/" in
  "$redline_container/"*)
    fail "selected hostile fixture path entered the canonical Redline container"
    ;;
esac

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

ambient_jankurai="$(command -v jankurai || true)"
if [[ -n "$ambient_jankurai" ]] \
  && ! jain_verify_governed_jankurai \
    "$ambient_jankurai" \
    "$JAIN_GOVERNED_JANKURAI_VERSION" \
    "$JAIN_GOVERNED_JANKURAI_SHA256" >/dev/null 2>&1
then
  if jankurai_bin >/dev/null 2>&1; then
    printf 'ambient wrong-digest Jankurai passed governed identity verification\n' >&2
    exit 1
  fi
  printf 'ambient wrong-digest Jankurai rejected: %s\n' "$ambient_jankurai"
fi

governed_source=""
while IFS= read -r candidate; do
  if jain_verify_governed_jankurai \
    "$candidate" \
    "$JAIN_GOVERNED_JANKURAI_VERSION" \
    "$JAIN_GOVERNED_JANKURAI_SHA256" >/dev/null 2>&1
  then
    governed_source="$candidate"
    break
  fi
done < <(type -a -p jankurai)
[[ -n "$governed_source" ]] \
  || fail "PATH does not contain the pinned governed Jankurai bytes required by this test"

mkdir "$tmp/governed-bin"
cp -- "$governed_source" "$tmp/governed-bin/jankurai"
chmod 0755 "$tmp/governed-bin/jankurai"
jain_verify_governed_jankurai \
  "$tmp/governed-bin/jankurai" \
  "$JAIN_GOVERNED_JANKURAI_VERSION" \
  "$JAIN_GOVERNED_JANKURAI_SHA256"

resolved="$(PATH="$tmp/governed-bin:/usr/bin:/bin" jankurai_bin)"
[[ "$resolved" == "$tmp/governed-bin/jankurai" ]] \
  || fail "positive governed Jankurai proof did not select the staged exact bytes"
jain_verify_governed_jankurai \
  "$resolved" "$JAIN_GOVERNED_JANKURAI_VERSION" "$JAIN_GOVERNED_JANKURAI_SHA256"
if PATH="$tmp/hostile-bin:$PATH" jankurai_bin >/dev/null 2>&1; then
  printf 'hostile PATH Jankurai passed governed identity verification\n' >&2
  exit 1
fi

missing_path="$tmp/missing-bin:/usr/bin:/bin"
if PATH="$missing_path" jankurai_bin >/dev/null 2>&1; then
  printf 'missing PATH Jankurai passed governed identity verification\n' >&2
  exit 1
fi

if grep -Fq '/home/ubuntu/.jeryu/bin/jankurai' "$ROOT_DIR/ops/ci/lib.sh"; then
  printf 'governed Jankurai selection still depends on the user home\n' >&2
  exit 1
fi

printf 'governed Jankurai hostile identity tests passed\n'
