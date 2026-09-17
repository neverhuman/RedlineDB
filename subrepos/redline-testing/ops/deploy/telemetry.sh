#!/usr/bin/env bash
set -euo pipefail

repo=""
sha=""
stage=""
ring_percent=""
format=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo) shift; repo="${1:-}" ;;
    --sha) shift; sha="${1:-}" ;;
    --stage) shift; stage="${1:-}" ;;
    --ring-percent) shift; ring_percent="${1:-}" ;;
    --format) shift; format="${1:-}" ;;
    *) printf 'telemetry: unknown arg: %s\n' "$1" >&2; exit 2 ;;
  esac
  shift
done

[[ -n "$repo" ]] || { printf 'telemetry: --repo is required\n' >&2; exit 2; }
[[ "$sha" =~ ^[0-9a-f]{40}$ ]] || { printf 'telemetry: --sha must be 40 hex\n' >&2; exit 2; }
[[ "$stage" == "prod" ]] || { printf 'telemetry: --stage must be prod\n' >&2; exit 2; }
[[ "$ring_percent" =~ ^(1|5|25|50|100)$ ]] || { printf 'telemetry: unsupported --ring-percent\n' >&2; exit 2; }
[[ "$format" == "jeryu-canary-v1" ]] || { printf 'telemetry: --format must be jeryu-canary-v1\n' >&2; exit 2; }

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

repo_slug_from_remote() {
  local url slug
  url="$(git remote get-url github 2>/dev/null || git remote get-url gh 2>/dev/null || git remote get-url origin 2>/dev/null || true)"
  slug="$(printf '%s' "$url" | sed -E 's#^git@github.com:##; s#^https://github.com/##; s#^ssh://git@github.com/##; s#\.git$##')"
  if [[ "$slug" == */* && "$slug" != http:* && "$slug" != ssh:* ]]; then
    printf '%s' "$slug"
  else
    printf 'neverhuman/%s' "$(basename "$repo_root")"
  fi
}

slug="${GITHUB_REPOSITORY:-$(repo_slug_from_remote)}"
store_root="${SIGNRAIL_STORE_ROOT:-${HOME}/.local/share/jeryu/signrail}"

cargo run --locked --quiet -p xtask -- telemetry \
  --repo "$repo" \
  --sha "$sha" \
  --ring-percent "$ring_percent" \
  --slug "$slug" \
  --store-root "$store_root"
