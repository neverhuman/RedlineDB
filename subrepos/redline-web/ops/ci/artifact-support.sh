#!/usr/bin/env bash
# Build and smoke the documented self-contained release binary, then emit
# exact-source evidence without turning that evidence into release authority.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"

artifact_relative="${REDLINE_ARTIFACT_SUPPORT_DIR:-target/artifact-support}"
[[ "$artifact_relative" == target/* \
  && "$artifact_relative" != */../* \
  && "$artifact_relative" != ../* \
  && "$artifact_relative" != */.. ]] \
  || fail "artifact support directory must remain beneath target/: ${artifact_relative}"
artifact_directory="$(realpath -m -- "$ROOT_DIR/$artifact_relative")"
[[ "$artifact_directory" == "$ROOT_DIR/target/"* ]] \
  || fail "artifact support directory escapes target/: ${artifact_directory}"

for tool in cargo git jq npm realpath sha256sum stat; do
  has "$tool" || fail "artifact support requires ${tool}"
done

source_commit="$(git rev-parse --verify 'HEAD^{commit}')"
source_tree="$(git rev-parse --verify 'HEAD^{tree}')"
[[ "$source_commit" =~ ^[0-9a-f]{40}$ && "$source_tree" =~ ^[0-9a-f]{40}$ ]] \
  || fail "artifact support requires full source commit and tree identities"
[[ -z "$(git status --porcelain=v1 --untracked-files=all)" ]] \
  || fail "artifact support requires a clean source checkout"

source_date_epoch="$(git show -s --format=%ct "$source_commit")"
[[ "$source_date_epoch" =~ ^[0-9]+$ ]] \
  || fail "artifact support source date must be an unsigned integer"
export SOURCE_DATE_EPOCH="$source_date_epoch"
export CARGO_INCREMENTAL=0

[[ ! -L "$ROOT_DIR/target" ]] \
  || fail "artifact support target root must not be a symlink"
if [[ -e "$artifact_directory" || -L "$artifact_directory" ]]; then
  [[ -d "$artifact_directory" && ! -L "$artifact_directory" ]] \
    || fail "artifact support output must be a physical directory"
fi
rm -rf -- "$artifact_directory"
mkdir -p -- "$artifact_directory"

# Vite empties its output directory before the build. Removing it explicitly
# prevents stale tracked assets from satisfying the later closed inventory.
rm -rf -- "$WEB_DIR/dist"
(
  cd "$WEB_DIR"
  npm ci --no-audit --no-fund
  npm run build -- --emptyOutDir
)

[[ -d "$WEB_DIR/dist" && ! -L "$WEB_DIR/dist" ]] \
  || fail "frontend release output is missing or not physical"
[[ -z "$(find "$WEB_DIR/dist" -mindepth 1 ! -type d ! -type f -print -quit)" ]] \
  || fail "frontend release output contains a symlink or special file"
[[ -z "$(find "$WEB_DIR/dist" -type f -empty -print -quit)" ]] \
  || fail "frontend release output contains an empty file"
mapfile -d '' frontend_files \
  < <(find "$WEB_DIR/dist" -type f -printf '%P\0' | LC_ALL=C sort -z)
[[ "${#frontend_files[@]}" -gt 0 ]] \
  || fail "frontend release output contains no nonempty regular files"

frontend_inventory="$artifact_directory/frontend-inventory.tsv"
: >"$frontend_inventory"
for relative in "${frontend_files[@]}"; do
  path="$WEB_DIR/dist/$relative"
  printf '%s\t%s\t%s\n' \
    "$relative" "$(stat -c %s -- "$path")" "$(jain_sha256 "$path")" \
    >>"$frontend_inventory"
done
frontend_content_sha256="$(jain_sha256 "$frontend_inventory")"

cargo build --locked --release --workspace
metadata="$(cargo metadata --locked --format-version 1 --no-deps)"
target_directory="$(jq -er '.target_directory | select(type == "string" and length > 0)' \
  <<<"$metadata")"
package_version="$(jq -er \
  '[.packages[] | select(.name == "redline-web-server")][0].version
   | select(type == "string" and length > 0)' <<<"$metadata")"
[[ -d "$target_directory" && ! -L "$target_directory" \
  && "$(realpath -e -- "$target_directory")" == "$target_directory" ]] \
  || fail "Cargo target directory is missing or not physical"

binary="$target_directory/release/redline-web"
[[ -f "$binary" && ! -L "$binary" && -x "$binary" && -s "$binary" \
  && "$(realpath -e -- "$binary")" == "$binary" ]] \
  || fail "release binary is missing, empty, non-executable, or not physical"
version_output="$("$binary" --version)"
[[ "$version_output" == "redline-web ${package_version}" ]] \
  || fail "release binary version smoke mismatch: ${version_output}"
"$binary" --help >"$artifact_directory/redline-web.help.txt"
[[ -s "$artifact_directory/redline-web.help.txt" ]] \
  || fail "release binary help smoke produced no output"

[[ "$(git rev-parse --verify 'HEAD^{commit}')" == "$source_commit" \
  && "$(git rev-parse --verify 'HEAD^{tree}')" == "$source_tree" \
  && -z "$(git status --porcelain=v1 --untracked-files=all)" ]] \
  || fail "source identity or cleanliness changed during artifact construction"

receipt="$artifact_directory/evidence.json"
receipt_tmp="$artifact_directory/.evidence.json.$$"
trap 'rm -f -- "$receipt_tmp"' EXIT
jq -n \
  --arg commit "$source_commit" \
  --arg tree "$source_tree" \
  --arg version "$package_version" \
  --arg source_date_epoch "$source_date_epoch" \
  --arg cargo_lock_sha256 "$(jain_sha256 "$ROOT_DIR/Cargo.lock")" \
  --arg frontend_lock_sha256 "$(jain_sha256 "$WEB_DIR/package-lock.json")" \
  --arg frontend_content_sha256 "$frontend_content_sha256" \
  --argjson frontend_file_count "${#frontend_files[@]}" \
  --arg binary_sha256 "$(jain_sha256 "$binary")" \
  --arg binary_size_bytes "$(stat -c %s -- "$binary")" \
  --arg smoke_version "$version_output" \
  '{
    schema_version: "redline-web.artifact-support/v1",
    status: "pass",
    purpose: "review_evidence_only",
    deployable: false,
    repo: "redline-web",
    version: $version,
    commit: $commit,
    tree: $tree,
    source_date_epoch: ($source_date_epoch | tonumber),
    cargo_lock_sha256: $cargo_lock_sha256,
    frontend_lock_sha256: $frontend_lock_sha256,
    frontend_content_sha256: $frontend_content_sha256,
    frontend_file_count: $frontend_file_count,
    binary_name: "redline-web",
    binary_sha256: $binary_sha256,
    binary_size_bytes: ($binary_size_bytes | tonumber),
    smoke_version: $smoke_version,
    source_clean: true
  }' >"$receipt_tmp"
jq -e '
  .schema_version == "redline-web.artifact-support/v1"
  and .status == "pass"
  and .source_clean == true
  and (.commit | test("^[0-9a-f]{40}$"))
  and (.tree | test("^[0-9a-f]{40}$"))
  and (.cargo_lock_sha256 | test("^[0-9a-f]{64}$"))
  and (.frontend_lock_sha256 | test("^[0-9a-f]{64}$"))
  and (.frontend_content_sha256 | test("^[0-9a-f]{64}$"))
  and (.binary_sha256 | test("^[0-9a-f]{64}$"))
  and (.frontend_file_count > 0)
  and (.binary_size_bytes > 0)
' "$receipt_tmp" >/dev/null
mv -- "$receipt_tmp" "$receipt"
trap - EXIT

[[ "$(git rev-parse --verify 'HEAD^{commit}')" == "$source_commit" \
  && "$(git rev-parse --verify 'HEAD^{tree}')" == "$source_tree" \
  && -z "$(git status --porcelain=v1 --untracked-files=all)" ]] \
  || fail "source identity or cleanliness changed while publishing artifact evidence"
log "artifact-support: physical redline-web ${package_version} passed"
