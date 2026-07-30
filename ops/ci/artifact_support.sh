#!/usr/bin/env bash
set -euo pipefail

workers="${1:-${WORKERS:-40}}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

out_dir="$repo_root/target/artifact-support"
sandbox_root=""
rm -rf "$out_dir"
mkdir -p "$out_dir/logs" "$out_dir/receipts" "$out_dir/bundles"

say() { printf '[artifact-support] %s\n' "$*" >&2; }
fail() {
  say "$*"
  exit 1
}

cleanup() {
  if [[ -n "$sandbox_root" && -d "$sandbox_root" ]]; then
    chmod -R u+w "$sandbox_root" 2>/dev/null || true
    rm -rf "$sandbox_root"
  fi
}
trap cleanup EXIT

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
    --mode='u+rwX,go+rX,go-w,a-s' \
    -cf - \
    -C "$evidence_dir" \
    context.json manifest.json receipts/local-ci.json \
    | gzip -n >"$bundle"
  bundle_sha="$(sha256sum "$bundle" | awk '{print $1}')"
  printf '%s  %s\n' "$bundle_sha" "$(basename "$bundle")" \
    >"$evidence_dir/bundles/artifact-support-evidence.tar.gz.sha256"
}

build_once() {
  local ordinal="$1"
  local entrypoint="$2"
  local source_commit="$3"
  local source_tree="$4"
  local build_root="$sandbox_root/build-$ordinal"
  local source_dir="$build_root/redline"
  local evidence_dir="$build_root/evidence"
  local rc=0

  mkdir -p "$build_root"
  git clone --no-local --no-hardlinks --no-tags "$repo_root" "$source_dir" \
    >"$build_root/clone.log" 2>&1
  git -C "$source_dir" checkout --detach "$source_commit" \
    >>"$build_root/clone.log" 2>&1
  [[ "$(git -C "$source_dir" rev-parse HEAD)" == "$source_commit" ]] \
    || fail "build $ordinal resolved the wrong source commit"
  [[ "$(git -C "$source_dir" rev-parse 'HEAD^{tree}')" == "$source_tree" ]] \
    || fail "build $ordinal resolved the wrong source tree"
  [[ -z "$(git -C "$source_dir" status --porcelain --untracked-files=all)" ]] \
    || fail "build $ordinal source is not exact-clean"
  [[ "$(git -C "$source_dir" worktree list --porcelain | grep -c '^worktree ')" -eq 1 ]] \
    || fail "build $ordinal source is not singly registered"

  mkdir -p "$evidence_dir/logs" "$evidence_dir/receipts" "$evidence_dir/bundles"
  (
    cd "$source_dir"
    run_ci "$entrypoint"
  ) >"$evidence_dir/logs/ci.log" 2>&1 || rc=$?
  if [[ "$rc" -ne 0 ]]; then
    install -m 0644 "$evidence_dir/logs/ci.log" "$out_dir/logs/build-$ordinal.log"
    say "independent build $ordinal failed; log: $out_dir/logs/build-$ordinal.log"
    return "$rc"
  fi
  (
    cd "$source_dir"
    write_json_files "$entrypoint" "$evidence_dir"
    bundle_evidence "$evidence_dir"
  )
}

publish_reproducible_evidence() {
  local source_commit="$1"
  local source_tree="$2"
  local first="$sandbox_root/build-1/evidence"
  local second="$sandbox_root/build-2/evidence"
  local relative_path bundle_sha first_log_sha second_log_sha

  for relative_path in \
    context.json \
    manifest.json \
    receipts/local-ci.json \
    bundles/artifact-support-evidence.tar.gz \
    bundles/artifact-support-evidence.tar.gz.sha256
  do
    if ! cmp -s "$first/$relative_path" "$second/$relative_path"; then
      fail "independent-build reproducibility mismatch: $relative_path"
    fi
  done

  install -m 0644 "$first/context.json" "$out_dir/context.json"
  install -m 0644 "$first/manifest.json" "$out_dir/manifest.json"
  install -m 0644 "$first/receipts/local-ci.json" "$out_dir/receipts/local-ci.json"
  install -m 0644 \
    "$first/bundles/artifact-support-evidence.tar.gz" \
    "$out_dir/bundles/artifact-support-evidence.tar.gz"
  install -m 0644 \
    "$first/bundles/artifact-support-evidence.tar.gz.sha256" \
    "$out_dir/bundles/artifact-support-evidence.tar.gz.sha256"
  install -m 0644 "$first/logs/ci.log" "$out_dir/logs/build-1.log"
  install -m 0644 "$second/logs/ci.log" "$out_dir/logs/build-2.log"

  bundle_sha="$(sha256sum "$out_dir/bundles/artifact-support-evidence.tar.gz" | awk '{print $1}')"
  first_log_sha="$(sha256sum "$out_dir/logs/build-1.log" | awk '{print $1}')"
  second_log_sha="$(sha256sum "$out_dir/logs/build-2.log" | awk '{print $1}')"
  jq -n \
    --arg source_commit "$source_commit" \
    --arg source_tree "$source_tree" \
    --arg bundle_sha "$bundle_sha" \
    --arg first_log_sha "$first_log_sha" \
    --arg second_log_sha "$second_log_sha" \
    '{
      schema_version: "redline.artifact-reproducibility/v1",
      status: "pass",
      source_commit: $source_commit,
      source_tree: $source_tree,
      bundle_sha256: $bundle_sha,
      build_count: 2,
      source_isolation: "two-disjoint-no-local-clones",
      normalized_bundle_excludes_raw_logs: true,
      raw_ci_log_sha256: [$first_log_sha, $second_log_sha]
    }' >"$out_dir/receipts/reproducibility.json"
}

entrypoint="$(pick_ci_entrypoint)" || { say "no supported CI entrypoint"; exit 91; }
[[ -z "$(git status --porcelain --untracked-files=all)" ]] \
  || fail "canonical source must be exact-clean before independent builds"
source_commit="$(git rev-parse HEAD)"
source_tree="$(git rev-parse 'HEAD^{tree}')"
sandbox_root="$(mktemp -d "${TMPDIR:-/tmp}/redline-artifact-support.XXXXXX")"

build_once 1 "$entrypoint" "$source_commit" "$source_tree"
build_once 2 "$entrypoint" "$source_commit" "$source_tree"
publish_reproducible_evidence "$source_commit" "$source_tree"
cleanup
sandbox_root=""
trap - EXIT
say "unsigned artifact support review evidence ready at $out_dir"
