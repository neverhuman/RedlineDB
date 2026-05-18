#!/usr/bin/env bash
# Activate the tracked Jankurai hooks for this clone.
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [ -z "$repo_root" ]; then
  echo "FAIL: run from inside a Git worktree" >&2
  exit 2
fi

cd "$repo_root"

hooks_path="${JANKURAI_HOOKS_PATH:-tools/jankurai-hooks}"
case "$hooks_path" in
  /*) hooks_dir="$hooks_path" ;;
  *) hooks_dir="$repo_root/$hooks_path" ;;
esac

for hook in pre-commit prepare-commit-msg; do
  if [ ! -x "$hooks_dir/$hook" ]; then
    echo "FAIL: expected executable hook at $hooks_path/$hook" >&2
    exit 2
  fi
done

git config core.hooksPath "$hooks_path"

git_dir="$(git rev-parse --git-dir)"
case "$git_dir" in
  /*) ;;
  *) git_dir="$repo_root/$git_dir" ;;
esac

jankurai_dir="$git_dir/jankurai"
env_file="$jankurai_dir/env"
env_tmp="$jankurai_dir/env.tmp"
legacy_dir="$git_dir/hooks"

mkdir -p "$jankurai_dir"
if [ -f "$env_file" ]; then
  grep -v -E '^export JANKURAI_(PRE_COMMIT|PREPARE_COMMIT_MSG)_CHAIN=' "$env_file" > "$env_tmp" || true
else
  : > "$env_tmp"
fi

chain_hook() {
  local var="$1"
  local legacy="$2"
  local managed="$3"

  if [ -x "$legacy" ] && ! cmp -s "$legacy" "$managed"; then
    printf 'export %s=%q\n' "$var" "$legacy" >> "$env_tmp"
    printf 'chained existing hook: %s\n' "$legacy"
  fi
}

chain_hook JANKURAI_PRE_COMMIT_CHAIN "$legacy_dir/pre-commit" "$hooks_dir/pre-commit"
chain_hook JANKURAI_PREPARE_COMMIT_MSG_CHAIN "$legacy_dir/prepare-commit-msg" "$hooks_dir/prepare-commit-msg"

if [ -s "$env_tmp" ]; then
  mv "$env_tmp" "$env_file"
else
  rm -f "$env_tmp"
fi

printf 'jankurai hooks installed: core.hooksPath=%s\n' "$hooks_path"
