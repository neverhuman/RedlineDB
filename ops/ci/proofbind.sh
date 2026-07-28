#!/usr/bin/env bash
# Jankurai 1.6.11 cannot classify a deleted changed path. Supply the complete
# non-deleted diff explicitly so generated evidence deletions cannot crash the
# governed proofbind lane.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"

has git || fail "git is required for exact-head proofbind evidence"
has jq || fail "jq is required for exact-head proofbind evidence"
[[ -z "$(git status --porcelain)" ]] || fail "proofbind requires a clean checkout"
JBIN="$(jankurai_bin)" || fail "governed Jankurai identity verification failed"
if [[ "${JAIN_RELEASE_CI:-0}" == "1" ]]; then
  : "${JAIN_CONTRACT_BASE_REF:?release proof base commit is required}"
  base_ref="$JAIN_CONTRACT_BASE_REF"
  [[ "$base_ref" =~ ^[0-9a-f]{40}$ ]] \
    || fail "release proof base must be a full lowercase commit: ${base_ref}"
else
  base_ref="${JANKURAI_BASE_REF:-origin/main}"
fi
base_commit="$(git rev-parse --verify "${base_ref}^{commit}" 2>/dev/null)" \
  || fail "proofbind base is not a local commit: ${base_ref}"
base_ref="$base_commit"
git merge-base --is-ancestor "$base_ref" HEAD \
  || fail "proofbind base is not an ancestor of HEAD: ${base_ref}"

changed_paths=()
while IFS= read -r -d '' path; do
  changed_paths+=("$path")
done < <(git diff --name-only --diff-filter=ACMRTUXB -z "$base_ref" --)
[[ "${#changed_paths[@]}" -gt 0 ]] \
  || fail "proofbind requires at least one non-deleted changed path"

changed_args=()
for path in "${changed_paths[@]}"; do
  changed_args+=(--changed "$path")
done
ensure_artifacts
witness="${ARTIFACT_DIR}/proofbind/surface-witness.json"
obligations="${ARTIFACT_DIR}/proofbind/obligations.json"
markdown="${ARTIFACT_DIR}/proofbind/proofbind.md"
"$JBIN" proofbind verify . "${changed_args[@]}" \
  --out "$witness" --obligations-out "$obligations" --md "$markdown"

changed_json="$(printf '%s\n' "${changed_paths[@]}" | jq -R . | jq -s .)"
expected_head="$(git rev-parse --short=7 HEAD)"
jq -e --arg head "$expected_head" --argjson changed "$changed_json" '
  .git_head == $head
  and .mode == "advisory"
  and ((.changed_paths | sort) == ($changed | sort))
  and (.summary.changed_surface_count >= 1)
' "$witness" >/dev/null
jq -e --arg head "$expected_head" '
  .git_head == $head
  and .mode == "advisory"
  and (.summary.total >= 0)
  and (.summary.missing >= 0)
' "$obligations" >/dev/null
log "proofbind: exact-head non-deleted surface evidence passed"
