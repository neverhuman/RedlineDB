#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

command -v jq >/dev/null 2>&1 || {
  printf 'jq is required for artifact evidence\n' >&2
  exit 1
}

source_commit="$(git rev-parse --verify 'HEAD^{commit}')"
source_tree="$(git rev-parse --verify 'HEAD^{tree}')"
[[ -z "$(git status --porcelain --untracked-files=all)" ]] || {
  printf 'artifact support requires a clean tracked and untracked source checkout\n' >&2
  exit 1
}
source_date_epoch="${SOURCE_DATE_EPOCH:-$(git show -s --format=%ct HEAD)}"
[[ "$source_date_epoch" =~ ^[0-9]+$ ]] || {
  printf 'SOURCE_DATE_EPOCH must be an unsigned integer\n' >&2
  exit 1
}
export SOURCE_DATE_EPOCH="$source_date_epoch"
export CARGO_INCREMENTAL=0

artifact_directory="${REDLINE_ARTIFACT_SUPPORT_DIR:-target/artifact-support}"
[[ "$artifact_directory" == target/* && "$artifact_directory" != *'/../'* \
  && "$artifact_directory" != ../* && "$artifact_directory" != */.. ]] || {
  printf 'artifact support directory must remain beneath target/: %s\n' \
    "$artifact_directory" >&2
  exit 1
}
artifact_resolved="$(realpath -m -- "$ROOT/$artifact_directory")"
[[ "$artifact_resolved" == "$ROOT/target/"* ]] || {
  printf 'artifact support directory escapes target/: %s\n' "$artifact_resolved" >&2
  exit 1
}

cargo build --release --locked --workspace --all-targets
target_directory="$(cargo metadata --locked --format-version 1 --no-deps \
  | jq -er '.target_directory | select(type == "string" and length > 0)')"
[[ -d "$target_directory" && ! -L "$target_directory" ]] || {
  printf 'Cargo target directory must be a physical directory: %s\n' "$target_directory" >&2
  exit 1
}
client_binary="$target_directory/release/redlinedb-client-smoke"
corpus_binary="$target_directory/release/db-shim-parity"
for binary in "$client_binary" "$corpus_binary"; do
  [[ -f "$binary" && ! -L "$binary" && -x "$binary" ]] || {
    printf 'missing physical release binary: %s\n' "$binary" >&2
    exit 1
  }
done
version="$(tr -d '\n' < VERSION)"
[[ "$version" == "4.1.0" ]]
package_directory="$artifact_directory/package"
mkdir -p "$artifact_directory"
rm -rf -- "$package_directory"
mkdir -p "$package_directory"
chmod 0755 "$package_directory"
install -m 0755 "$client_binary" \
  "$package_directory/redlinedb-client-smoke"
install -m 0755 "$corpus_binary" "$package_directory/db-shim-parity"
install -m 0644 docs/support-archive.md "$package_directory/SUPPORT-ARCHIVE.md"
install -m 0644 db/backend-contract.toml "$package_directory/backend-contract.toml"
artifact="$artifact_directory/redline-central-v${version}.tar.gz"
artifact_tmp="${artifact}.tmp"
rm -f -- "$artifact_tmp"
trap 'rm -f -- "$artifact_tmp"' EXIT
archive_entries=(
  ./
  ./SUPPORT-ARCHIVE.md
  ./backend-contract.toml
  ./db-shim-parity
  ./redlinedb-client-smoke
)
(
  cd "$package_directory"
  printf '%s\0' "${archive_entries[@]}" \
    | tar --create --file=- --no-recursion --sort=name --format=gnu --null \
      --mtime="@${source_date_epoch}" --owner=0 --group=0 --numeric-owner \
      --mode='u+rwX,go+rX,go-w' --files-from=-
) | gzip -n -9 > "$artifact_tmp"
mv -- "$artifact_tmp" "$artifact"
trap - EXIT
[[ "$(git rev-parse --verify 'HEAD^{commit}')" == "$source_commit" \
  && "$(git rev-parse --verify 'HEAD^{tree}')" == "$source_tree" \
  && -z "$(git status --porcelain --untracked-files=all)" ]] || {
  printf 'source identity changed during artifact construction\n' >&2
  exit 1
}
jq -n \
  --arg repo "redline-central" \
  --arg version "$version" \
  --arg commit "$source_commit" \
  --arg tree "$source_tree" \
  --arg lock_sha256 "$(sha256sum Cargo.lock | awk '{print $1}')" \
  --arg backend_contract_sha256 "$(sha256sum db/backend-contract.toml | awk '{print $1}')" \
  --arg artifact_sha256 "$(sha256sum "$artifact" | awk '{print $1}')" \
  --arg source_date_epoch "$source_date_epoch" \
  '{schema_version:"redline-central.artifact-support/v1",repo:$repo,version:$version,
    purpose:"review_evidence_only",deployable:false,commit:$commit,tree:$tree,
    source_date_epoch:($source_date_epoch | tonumber),lock_sha256:$lock_sha256,
    backend_contract_sha256:$backend_contract_sha256,artifact_sha256:$artifact_sha256,status:"pass"}' \
  > "$artifact_directory/evidence.json"
printf 'artifact support ok: redline-central v%s\n' "$version"
