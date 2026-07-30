#!/usr/bin/env bash
set -euo pipefail

workers="${1:-${WORKERS:-40}}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

out_dir="$repo_root/target/artifact-support"
repro_dir="$repo_root/target/artifact-support-repro"
rm -rf "$out_dir" "$repro_dir"
mkdir -p "$out_dir/logs" "$out_dir/receipts" "$out_dir/bundles"

say() { printf '[artifact-support] %s\n' "$*" >&2; }

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
  local entrypoint="$1"
  local destination="$2"
  bash tools/evidence-processor/run.sh artifact-metadata \
    --out-dir "$destination" \
    --entrypoint "$entrypoint" \
    --workers "$workers"
}

bundle_evidence() {
  local evidence_dir="$1"
  local bundle source_epoch bundle_sha
  bundle="$evidence_dir/bundles/artifact-support-evidence.tar.gz"
  source_epoch="$(git show -s --format=%ct HEAD)"
  mkdir -p "$evidence_dir/bundles"
  tar --sort=name \
    --mtime="@${source_epoch}" \
    --owner=0 \
    --group=0 \
    --numeric-owner \
    -cf - \
    -C "$evidence_dir" context.json manifest.json logs receipts \
    | gzip -n >"$bundle"
  bundle_sha="$(sha256sum "$bundle" | awk '{print $1}')"
  printf '%s  %s\n' "$bundle_sha" "$(basename "$bundle")" \
    >"$evidence_dir/bundles/artifact-support-evidence.tar.gz.sha256"
}

verify_reproducible_evidence() {
  local entrypoint="$1"
  local relative_path bundle_sha

  mkdir -p "$repro_dir/logs" "$repro_dir/receipts" "$repro_dir/bundles"
  install -m 0644 "$out_dir/logs/ci.log" "$repro_dir/logs/ci.log"
  write_json_files "$entrypoint" "$repro_dir"
  bundle_evidence "$repro_dir"

  for relative_path in \
    context.json \
    manifest.json \
    receipts/local-ci.json \
    bundles/artifact-support-evidence.tar.gz \
    bundles/artifact-support-evidence.tar.gz.sha256
  do
    if ! cmp -s "$out_dir/$relative_path" "$repro_dir/$relative_path"; then
      say "reproducibility mismatch: $relative_path"
      return 92
    fi
  done

  bundle_sha="$(sha256sum "$out_dir/bundles/artifact-support-evidence.tar.gz" | awk '{print $1}')"
  printf '{\n  "schema_version": "redline.artifact-reproducibility/v1",\n  "status": "pass",\n  "source_commit": "%s",\n  "bundle_sha256": "%s",\n  "build_count": 2\n}\n' \
    "$(git rev-parse HEAD)" \
    "$bundle_sha" \
    >"$out_dir/receipts/reproducibility.json"
  rm -rf "$repro_dir"
}

entrypoint="$(pick_ci_entrypoint)" || { say "no supported CI entrypoint"; exit 91; }
if run_ci "$entrypoint" >"$out_dir/logs/ci.log" 2>&1; then
  write_json_files "$entrypoint" "$out_dir"
  bundle_evidence "$out_dir"
  verify_reproducible_evidence "$entrypoint"
  say "unsigned artifact support review evidence ready at $out_dir"
else
  rc=$?
  say "CI failed; log: $out_dir/logs/ci.log"
  exit "$rc"
fi
