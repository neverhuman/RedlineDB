#!/usr/bin/env bash
set -euo pipefail

workers="${1:-${WORKERS:-40}}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

out_dir="$repo_root/target/artifact-support"
rm -rf "$out_dir"
mkdir -p "$out_dir/logs" "$out_dir/receipts" "$out_dir/bundles"

say() { printf '[artifact-support] %s\n' "$*" >&2; }

current_sha() {
  git rev-parse HEAD
}

just_has() {
  command -v just >/dev/null 2>&1 || return 1
  [[ -f justfile || -f Justfile || -f .justfile ]] || return 1
  just --summary 2>/dev/null | tr ' ' '\n' | grep -qx "$1"
}

pick_ci_entrypoint() {
  if [[ -x ./ci-fast-push.sh ]]; then
    printf './ci-fast-push.sh --no-push --ci'
  elif [[ -f ops/ci/pr-ci.sh ]]; then
    printf 'bash ops/ci/pr-ci.sh'
  elif [[ -f scripts/ci-local.sh ]]; then
    printf 'bash scripts/ci-local.sh'
  elif just_has fast; then
    printf 'just fast'
  elif just_has check; then
    printf 'just check'
  elif just_has test; then
    printf 'just test'
  elif [[ -f Cargo.toml ]] && cargo nextest --version >/dev/null 2>&1; then
    printf 'cargo nextest run --workspace --no-fail-fast'
  elif [[ -f Cargo.toml ]]; then
    printf 'cargo test --workspace --no-fail-fast'
  elif [[ -f package.json ]]; then
    printf 'npm test'
  else
    return 1
  fi
}

run_ci() {
  local entrypoint="$1"
  say "ci-entrypoint: $entrypoint"
  case "$entrypoint" in
    './ci-fast-push.sh --no-push --ci')
      WORKERS="$workers" ./ci-fast-push.sh --no-push --ci
      ;;
    'bash ops/ci/pr-ci.sh')
      bash ops/ci/pr-ci.sh
      ;;
    'bash scripts/ci-local.sh')
      bash scripts/ci-local.sh
      ;;
    'just fast')
      just fast
      ;;
    'just check')
      just check
      ;;
    'just test')
      just test
      ;;
    'cargo nextest run --workspace --no-fail-fast')
      cargo nextest run --workspace --no-fail-fast
      ;;
    'cargo test --workspace --no-fail-fast')
      cargo test --workspace --no-fail-fast
      ;;
    'npm test')
      if [[ -f package-lock.json ]]; then
        npm ci --no-audit --no-fund
      fi
      npm test
      ;;
    *)
      printf 'unsupported CI entrypoint: %s\n' "$entrypoint" >&2
      return 91
      ;;
  esac
}

write_json_files() {
  local entrypoint="$1" sha tree generated_at
  sha="$(current_sha)"
  tree="$(git rev-parse 'HEAD^{tree}')"
  generated_at="$(git show -s --format=%cI HEAD)"
  cargo run --locked --quiet -p xtask -- artifact-support-json \
    --out-dir "$out_dir" \
    --entrypoint "$entrypoint" \
    --sha "$sha" \
    --tree "$tree" \
    --generated-at "$generated_at" \
    --workers "$workers"
}

bundle_evidence() {
  local bundle source_epoch bundle_sha
  bundle="$out_dir/bundles/artifact-support-evidence.tar.gz"
  source_epoch="$(git show -s --format=%ct HEAD)"
  tar --sort=name \
    --mtime="@${source_epoch}" \
    --owner=0 \
    --group=0 \
    --numeric-owner \
    -cf - \
    -C "$out_dir" context.json manifest.json logs receipts \
    | gzip -n >"$bundle"
  bundle_sha="$(sha256sum "$bundle" | awk '{print $1}')"
  printf '%s  %s\n' "$bundle_sha" "$(basename "$bundle")" \
    >"$out_dir/bundles/artifact-support-evidence.tar.gz.sha256"
}

entrypoint="$(pick_ci_entrypoint)" || { say "no supported CI entrypoint"; exit 91; }
if run_ci "$entrypoint" >"$out_dir/logs/ci.log" 2>&1; then
  write_json_files "$entrypoint"
  bundle_evidence
  say "unsigned artifact support review evidence ready at $out_dir"
else
  rc=$?
  say "CI failed; log: $out_dir/logs/ci.log"
  exit "$rc"
fi
