#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$root"
for tool in git cargo rustc cc pkg-config; do
  command -v "$tool" >/dev/null || { printf 'missing prerequisite: %s\n' "$tool" >&2; exit 1; }
done
cargo build --locked --release -p redlinedb-cli
printf 'Built target/release/redlinedb-cli. Install with ./scripts/install-from-source.sh\n'
