#!/usr/bin/env bash
# Non-test portions of the required lane, split around the one typed Rust
# test/coverage execution. This is an internal CI interface, not a caller
# controlled skip switch.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

say() { printf '[required:jain-split-ops] %s\n' "$*" >&2; }

case "${1:-}" in
  pre-rust-tests)
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
    bash ops/ci/cargo-lock-closure-test.sh
    bash ops/ci/native-runtime-test.sh
    bash ops/ci/pnpm-runtime-test.sh
    bash ops/ci/native-materializer-test.sh
    bash ops/ci/host-ci-integrity-test.sh
    if [[ "${JAIN_HOST_CI_NETWORK_ISOLATED:-0}" == 1 ]]; then
      bash ops/ci/host-ci-isolated-contract-test.sh
    else
      bash ops/ci/split-host-ci-integrity-test.sh
    fi
    bash ops/ci/pinned-advisory-test.sh

    say 'Rust: format, canonical manifest, source coverage, and local-Jeryu policy'
    cargo fmt -- --check
    ;;
  post-rust-tests)
    cargo run --locked --quiet -- validate-manifest --manifest repos.manifest.toml --check-paths --check-derived
    cargo run --locked --quiet -- source-coverage --manifest repos.manifest.toml
    local_jeryu_args=(validate-local-jeryu --manifest repos.manifest.toml)
    if [[ "${JAIN_HOST_CI_NETWORK_ISOLATED:-0}" == 1 ]]; then
      # Exact-head CI is source authority, not a mutable host-checkout census. The
      # host preflight/convergence lane verifies canonical checkout remotes, while
      # the isolated worker consumes the authenticated outer-family projection.
      local_jeryu_args+=(--skip-remotes --sealed-outer-projection)
    fi
    cargo run --locked --quiet -- "${local_jeryu_args[@]}"
    if [[ "${JAIN_HOST_CI_NETWORK_ISOLATED:-0}" != 1 ]]; then
      bash ops/ci/registered-nested-authority-test.sh
    fi
    cargo run --locked --quiet -- python-boundary
    ;;
  *)
    printf 'usage: %s pre-rust-tests|post-rust-tests\n' "$0" >&2
    exit 2
    ;;
esac
