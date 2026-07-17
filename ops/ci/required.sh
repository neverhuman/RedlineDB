#!/usr/bin/env bash
# Required lane for the Rust jain-split-ops control plane. Python is not a
# control-plane runtime dependency; parity-only Python lives in bounded test
# surfaces outside this operational lane.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

say() { printf '[required:jain-split-ops] %s\n' "$*" >&2; }

for tool in bash cargo file git jq ldd realpath rg rsync sha256sum shellcheck xargs; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'required tool is unavailable: %s\n' "$tool" >&2
    exit 1
  }
done

say 'shell: bash -n + shellcheck (error level) on all ops shell scripts'
mapfile -t sh_files < <(find ops -type f -name '*.sh' | sort)
for f in "${sh_files[@]}"; do bash -n "$f"; done
shellcheck -S error "${sh_files[@]}"
bash ops/ci/native-runtime-test.sh
bash ops/ci/native-materializer-test.sh
bash ops/ci/host-ci-integrity-test.sh
if [[ "${JAIN_HOST_CI_NETWORK_ISOLATED:-0}" == 1 ]]; then
  bash ops/ci/host-ci-isolated-contract-test.sh
else
  bash ops/ci/split-host-ci-integrity-test.sh
fi
bash ops/ci/pinned-advisory-test.sh

say 'Rust: format, tests, canonical manifest, source coverage, and local-Jeryu policy'
cargo fmt -- --check
cargo test --locked
cargo run --locked --quiet -- validate-manifest --manifest repos.manifest.toml --check-paths --check-derived
cargo run --locked --quiet -- source-coverage --manifest repos.manifest.toml
local_jeryu_args=(validate-local-jeryu --manifest repos.manifest.toml)
if [[ "${JAIN_HOST_CI_NETWORK_ISOLATED:-0}" == 1 ]]; then
  # Exact-head CI is source authority, not a mutable host-checkout census. The
  # host preflight/convergence lane verifies canonical checkout remotes.
  local_jeryu_args+=(--skip-remotes)
fi
cargo run --locked --quiet -- "${local_jeryu_args[@]}"
cargo run --locked --quiet -- python-boundary

printf 'required ok: jain-split-ops\n'
