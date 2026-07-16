#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=ops/ci/lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
cargo install --locked cargo-audit --version "$CARGO_AUDIT_VERSION"
cargo install --locked cargo-deny --version "$CARGO_DENY_VERSION"
cargo install --locked zizmor --version "$ZIZMOR_VERSION"
GOBIN="$HOME/.local/bin" go install "github.com/rhysd/actionlint/cmd/actionlint@v$ACTIONLINT_VERSION"
GOBIN="$HOME/.local/bin" go install "github.com/gitleaks/gitleaks/v8@v$GITLEAKS_VERSION"
GOBIN="$HOME/.local/bin" go install "github.com/anchore/syft/cmd/syft@v$SYFT_VERSION"
