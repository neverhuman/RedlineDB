#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/security-tools.sh
source "$repo_root/ops/ci/security-tools.sh"

tmp="$(mktemp -d "${TMPDIR:-/tmp}/redline-core-security-inputs.XXXXXX")"
trap 'rm -rf -- "$tmp"' EXIT
fixture="$tmp/tool"
printf '#!/usr/bin/env bash\nprintf "fixture 1.0\\n"\n' >"$fixture"
chmod 0555 "$fixture"
digest="$(sha256sum "$fixture" | awk '{print $1}')"
redline_validate_security_tool fixture "$fixture" "fixture 1.0" "$digest"

expect_rejected() {
    local label="$1"
    shift
    if "$@" >/dev/null 2>&1; then
        printf 'security input unexpectedly accepted: %s\n' "$label" >&2
        exit 1
    fi
}

expect_rejected missing redline_validate_security_tool fixture "$tmp/missing" "fixture 1.0" "$digest"
ln -s -- "$fixture" "$tmp/symlink"
expect_rejected symlink redline_validate_security_tool fixture "$tmp/symlink" "fixture 1.0" "$digest"
ln -- "$fixture" "$tmp/hardlink"
expect_rejected hardlink redline_validate_security_tool fixture "$fixture" "fixture 1.0" "$digest"
rm -- "$tmp/hardlink"
expect_rejected digest redline_validate_security_tool fixture "$fixture" "fixture 1.0" \
    0000000000000000000000000000000000000000000000000000000000000000
expect_rejected version redline_validate_security_tool fixture "$fixture" "fixture 2.0" "$digest"
expect_rejected unsealed-release env JAIN_RELEASE_CI=1 bash -c \
    'source "$1"; redline_validate_security_tool fixture "$2" "fixture 1.0" "$3"' \
    bash "$repo_root/ops/ci/security-tools.sh" "$fixture" "$digest"

printf 'governed security tool hostiles passed: missing symlink hardlink digest version custody\n'
