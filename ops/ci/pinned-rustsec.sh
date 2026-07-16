#!/usr/bin/env bash
# Materialize one reviewed RustSec snapshot without trusting or mutating its worktree.

JAIN_RUSTSEC_COMMIT="9f3e138091487e69144f536d36976e427a7a3307"
JAIN_RUSTSEC_TREE="c33f1047906505cabcec7e21f2d99db5c6de8852"
JAIN_RUSTSEC_ARCHIVE_SHA256="08098d56e4349bd8fc08e8be06ba057e481ef547c859194fc538f0acbd0be63c"
JAIN_RUSTSEC_OBJECT_SOURCE="/home/ubuntu/.cargo/advisory-db"

jain_materialize_pinned_rustsec() (
  set -euo pipefail

  local root source target parent temporary resolved commit tree archive_sha
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  source="${JAIN_RUSTSEC_OBJECT_SOURCE_OVERRIDE:-$JAIN_RUSTSEC_OBJECT_SOURCE}"
  target="$root/target/jankurai/security/rustsec-db"
  parent="$(dirname "$target")"

  for command in git jq mktemp realpath sha256sum tar; do
    command -v "$command" >/dev/null 2>&1 || {
      printf 'missing RustSec materialization command: %s\n' "$command" >&2
      exit 1
    }
  done
  [[ -d "$source" && ! -L "$source" && -d "$source/.git" && ! -L "$source/.git" ]] || {
    printf 'RustSec object source must be a physical Git repository: %s\n' "$source" >&2
    exit 1
  }
  resolved="$(realpath -e -- "$source")"
  [[ "$resolved" == "$source" ]] || {
    printf 'RustSec object source resolved outside its exact path: %s\n' "$resolved" >&2
    exit 1
  }

  commit="$(git -c core.hooksPath=/dev/null -c diff.external= -C "$source" \
    rev-parse --verify "$JAIN_RUSTSEC_COMMIT^{commit}")"
  tree="$(git -c core.hooksPath=/dev/null -c diff.external= -C "$source" \
    rev-parse --verify "$JAIN_RUSTSEC_COMMIT^{tree}")"
  [[ "$commit" == "$JAIN_RUSTSEC_COMMIT" && "$tree" == "$JAIN_RUSTSEC_TREE" ]] || {
    printf 'RustSec commit/tree identity mismatch\n' >&2
    exit 1
  }

  mkdir -p "$parent"
  temporary="$(mktemp -d "$parent/.rustsec-materialize.XXXXXX")"
  trap 'rm -rf -- "$temporary"' EXIT
  git -c core.hooksPath=/dev/null -c diff.external= -C "$source" archive \
    --format=tar --output="$temporary/rustsec.tar" "$JAIN_RUSTSEC_COMMIT"
  archive_sha="$(sha256sum -- "$temporary/rustsec.tar" | awk '{print $1}')"
  [[ "$archive_sha" == "$JAIN_RUSTSEC_ARCHIVE_SHA256" ]] || {
    printf 'RustSec archive digest mismatch: %s\n' "$archive_sha" >&2
    exit 1
  }

  mkdir "$temporary/db"
  tar --extract --file="$temporary/rustsec.tar" --directory="$temporary/db" \
    --no-same-owner --no-same-permissions
  [[ -d "$temporary/db/crates" && -f "$temporary/db/support.toml" ]] || {
    printf 'RustSec archive is missing its advisory database contract\n' >&2
    exit 1
  }
  [[ -z "$(/usr/bin/find "$temporary/db" -type l -print -quit)" ]] || {
    printf 'RustSec archive contains a symbolic link\n' >&2
    exit 1
  }

  if [[ -e "$target" || -L "$target" ]]; then
    [[ -d "$target" && ! -L "$target" \
      && -z "$(/usr/bin/find "$target" -type l -print -quit)" ]] || {
      printf 'refusing to replace unsafe RustSec target: %s\n' "$target" >&2
      exit 1
    }
    rm -rf -- "$target"
  fi
  mv -- "$temporary/db" "$target"
  jq -n \
    --arg schema_version 'redline-central.rustsec-snapshot/v1' \
    --arg commit "$commit" --arg tree "$tree" --arg archive_sha256 "$archive_sha" \
    '{schema_version:$schema_version,commit:$commit,tree:$tree,
      archive_sha256:$archive_sha256,status:"pass"}' \
    > "$parent/rustsec-snapshot.json"
)
