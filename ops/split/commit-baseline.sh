#!/usr/bin/env bash
# Commit regenerated Jain split baselines in reviewable groups.
set -euo pipefail

split="${JAIN_SPLIT_ROOT:-/home/ubuntu/jain-split}"
manifest="${JAIN_SPLIT_MANIFEST:-${split}/repos.manifest.toml}"

mapfile -t rows < <(bash "$(dirname "\${BASH_SOURCE[0]}")/manifest.sh" --manifest "$manifest" | cut -d'|' -f1-2)

commit_group() {
  local repo_dir="$1" message="$2"; shift 2
  local pathspecs=("$@")
  git -C "$repo_dir" add -A -- "${pathspecs[@]}" 2>/dev/null || true
  if ! git -C "$repo_dir" diff --cached --quiet; then
    git -C "$repo_dir" -c user.name="Jain Split Bot" -c user.email="split-bot@localhost" \
      commit -q -m "$message"
    printf '  committed: %s\n' "$message"
  fi
}

for row in "${rows[@]}"; do
  IFS='|' read -r repo path <<<"$row"
  [[ -d "$path/.git" ]] || { printf 'SKIP %s (no .git)\n' "$repo"; continue; }
  printf '== %s\n' "$repo"

  commit_group "$path" "ci(split): refresh Jain proof lanes and workflows [skip-version]" \
    ".github" "ops/ci" "ops/git-hooks" "Justfile" "scripts/ci-local.sh" "scripts/ci-doctor.sh"

  commit_group "$path" "chore(split): refresh agent metadata and provenance [skip-version]" \
    "agent" "docs" "AGENTS.md" "README.md" "SPLIT.md" "CHANGELOG.md" "VERSION" \
    "ops/AGENTS.md" "contracts/MIRROR.md" "repos.manifest.toml" "family.lock" "jain-split.lock.toml"

  commit_group "$path" "chore(split): refresh Jain source baseline [skip-version]" "."

  left="$(git -C "$path" status --porcelain | wc -l)"
  printf '  remaining dirty: %s\n' "$left"
done
