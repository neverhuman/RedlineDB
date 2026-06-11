#!/usr/bin/env bash
# Contract drift check for the RedlineDB hub.
# Re-derives the install URL template from install.sh and verifies it is
# well-formed (contains a dynamic ${VERSION} variable and a releases/download path).
# This is the public-API drift gate for the hub's only external contract surface.
set -Eeuo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib.sh"
cd "$(repo_root)"

log_step "contract drift: verify install.sh URL template is well-formed"

# Re-derive the canonical URL template from install.sh. Match a releases/download
# line that interpolates a shell variable in any supported style: ${VERSION},
# $REPO, or $tag (the verification below accepts the same forms).
derived="$(grep -E 'releases/download' install.sh | grep -E '\$\{?[A-Za-z_]' | head -1 | sed 's/^ *[a-zA-Z_]*=//; s/["\x27]//g; s/^ *//; s/ *$//')"
if [ -z "$derived" ]; then
    die "$ERR_CONTRACT_MISMATCH" "install.sh: cannot derive URL template (missing releases/download line with a \${VERSION} variable)"
fi

log_ok "URL template: $derived"

# Verify the derived URL has the required structure.
if ! echo "$derived" | grep -q 'releases/download'; then
    die "$ERR_CONTRACT_MISMATCH" "install.sh URL is missing 'releases/download' — see docs/testing.md#agent-repair-hints"
fi
if ! echo "$derived" | grep -qE '\$\{[A-Z_]+\}|\$[A-Z_]+'; then
    die "$ERR_CONTRACT_MISMATCH" "install.sh URL must use a dynamic variable (e.g. \${VERSION}) — see docs/testing.md#agent-repair-hints"
fi

log_ok "contract drift check passed — URL template is well-formed"
