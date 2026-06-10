#!/usr/bin/env bash
# Doctor: list every tool the ops/ci lanes depend on and whether it is present,
# so a developer can confirm their local environment matches CI.
set -Eeuo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

printf '[redline-ci] doctor: repository %s\n' "$ROOT_DIR"

for tool in bash node npm cargo just gitleaks cargo-audit cargo-deny zizmor syft actionlint jankurai python3; do
  if command -v "$tool" >/dev/null 2>&1; then
    version="$("$tool" --version 2>/dev/null | head -n 1 || true)"
    printf '[redline-ci] tool %-12s %s\n' "$tool" "${version:-present}"
  else
    printf '[redline-ci][warn] tool %-12s missing\n' "$tool"
  fi
done

printf '[redline-ci] manifests:'
for file in Cargo.toml Cargo.lock apps/web/package.json apps/web/package-lock.json deny.toml gitleaks.toml; do
  [[ -e "$file" ]] && printf ' %s' "$file"
done
printf '\n'
