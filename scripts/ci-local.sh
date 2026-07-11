#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
case "${1:-validate}" in
  validate|fast) exec "$repo_root/redlinectl" validate ;;
  required|family-ci) exec "$repo_root/redlinectl" family-ci ;;
  doctor) exec "$repo_root/redlinectl" doctor ;;
  *) printf 'usage: %s {validate|fast|required|family-ci|doctor}\n' "$0" >&2; exit 64 ;;
esac
