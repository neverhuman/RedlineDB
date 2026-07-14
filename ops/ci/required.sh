#!/usr/bin/env bash
# Required lane for the Rust jain-split-ops control plane. Python is not a
# control-plane runtime dependency; parity-only Python lives in bounded test
# surfaces outside this operational lane.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

say() { printf '[required:jain-split-ops] %s\n' "$*" >&2; }

for tool in bash cargo git jq rg sha256sum shellcheck; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'required tool is unavailable: %s\n' "$tool" >&2
    exit 1
  }
done

say 'shell: bash -n + shellcheck (error level) on all ops shell scripts'
mapfile -t sh_files < <(find ops -type f -name '*.sh' | sort)
for f in "${sh_files[@]}"; do bash -n "$f"; done
shellcheck -S error "${sh_files[@]}"

say 'Rust: format, tests, canonical manifest, source coverage, and local-Jeryu policy'
cargo fmt -- --check
cargo test --locked
cargo run --locked --quiet -- validate-manifest --manifest repos.manifest.toml --check-paths --check-derived
cargo run --locked --quiet -- source-coverage --manifest repos.manifest.toml
cargo run --locked --quiet -- validate-local-jeryu --manifest repos.manifest.toml
cargo run --locked --quiet -- python-boundary --manifest repos.manifest.toml

printf 'required ok: jain-split-ops\n'
