#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

source ops/ci/lib.sh
for command in cargo jq rg sha256sum realpath; do
  require_cmd "$command"
done

JANKURAI_BIN="/home/ubuntu/.jeryu/bin/jankurai"
JANKURAI_VERSION="1.6.11"
JANKURAI_SHA256="fdb42e5fa7d9851c0729e59bf1e582c895aa9cfc03a7175b420c6025d2fd014e"
[[ -f "$JANKURAI_BIN" && ! -L "$JANKURAI_BIN" && -x "$JANKURAI_BIN" ]]
[[ "$(realpath -e -- "$JANKURAI_BIN")" == "$JANKURAI_BIN" ]]
[[ "$($JANKURAI_BIN --version)" == "jankurai $JANKURAI_VERSION" ]]
[[ "$(sha256sum -- "$JANKURAI_BIN" | awk '{print $1}')" == "$JANKURAI_SHA256" ]]

for script in ops/ci/*.sh scripts/ci-local.sh scripts/ci-doctor.sh; do
  bash -n "$script"
done
[[ -x ops/ci/jankurai.sh && -x ops/ci/required.sh && -x scripts/ci-local.sh ]]
grep -Fq 'bash ops/ci/jankurai.sh' .github/workflows/jankurai.yml
[[ "$(grep -Fc 'uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683' \
  .github/workflows/jankurai.yml)" -eq 4 ]]
[[ "$(grep -Fc 'persist-credentials: false' .github/workflows/jankurai.yml)" -eq 4 ]]
grep -Fq 'bash ops/ci/pinned-rustsec-test.sh' ops/ci/security.sh
grep -Fq 'cargo audit --db target/jankurai/security/rustsec-db --no-fetch --json' \
  ops/ci/security.sh
grep -Fq 'JAIN_RUSTSEC_COMMIT="9f3e138091487e69144f536d36976e427a7a3307"' \
  ops/ci/pinned-rustsec.sh
grep -Fq 'JAIN_RUSTSEC_ARCHIVE_SHA256="08098d56e4349bd8fc08e8be06ba057e481ef547c859194fc538f0acbd0be63c"' \
  ops/ci/pinned-rustsec.sh
grep -Fq 'zizmor --offline --format json' ops/ci/security.sh
if grep -Fq -- '--no-exit-codes' ops/ci/security.sh; then
  printf 'security lane must not suppress Zizmor findings\n' >&2
  exit 1
fi
printf 'ci doctor ok: governed Jankurai and local lane dispatch\n'
