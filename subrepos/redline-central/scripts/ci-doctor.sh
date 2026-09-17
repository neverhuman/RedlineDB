#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

source ops/ci/lib.sh
for command in cargo jq rg sha256sum realpath; do
  require_cmd "$command"
done

require_governed_jankurai

for script in ops/ci/*.sh scripts/ci-local.sh scripts/ci-doctor.sh; do
  bash -n "$script"
done
[[ -x ops/ci/jankurai.sh && -x ops/ci/required.sh && -x scripts/ci-local.sh ]]
grep -Fq 'bash ops/ci/jankurai.sh' .github/workflows/jankurai.yml
grep -Fq 'JANKURAI_UPDATE_REVIEWED=1' scripts/ci-local.sh
grep -Fq 'bash ops/ci/governed-jankurai-test.sh' ops/ci/required.sh
[[ "$(grep -Fc 'uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683' \
  .github/workflows/jankurai.yml)" -eq 4 ]]
[[ "$(grep -Fc 'persist-credentials: false' .github/workflows/jankurai.yml)" -eq 4 ]]
grep -Fq 'bash ops/ci/pinned-rustsec-test.sh' ops/ci/security.sh
grep -Fq "cargo audit --db \"\$JAIN_RESOLVED_ADVISORY_DB\" --no-fetch --json" \
  ops/ci/security.sh
grep -Fq 'JAIN_PINNED_ADVISORY_DB' \
  ops/ci/pinned-rustsec.sh
grep -Fq 'JAIN_PINNED_ADVISORY_COMMIT' \
  ops/ci/pinned-rustsec.sh
grep -Fq 'JAIN_RESOLVED_ADVISORY_TREE' ops/ci/pinned-rustsec.sh
grep -Fq 'bash ops/ci/family-release-test.sh' ops/ci/required.sh
if grep -Fq 'REDLINE_CORPUS_DSN' ops/ci/fast.sh; then
  printf 'protected fast lane must not require an external Redline DSN\n' >&2
  exit 1
fi
grep -Fq 'cargo metadata --locked --format-version 1 --no-deps' ops/ci/artifact-support.sh
if grep -Eq 'install .*target/release/(redlinedb-client-smoke|db-shim-parity)' \
  ops/ci/artifact-support.sh; then
  printf 'artifact lane must honor the exact Cargo target directory\n' >&2
  exit 1
fi
grep -Fq 'export SOURCE_DATE_EPOCH=' ops/ci/artifact-support.sh
grep -Fq "CARGO_TARGET_DIR=\"\$first_target\"" ops/ci/artifact-support-test.sh
grep -Fq "CARGO_TARGET_DIR=\"\$second_target\"" ops/ci/artifact-support-test.sh
grep -Fq 'zizmor --offline --format json' ops/ci/security.sh
if grep -Fq -- '--no-exit-codes' ops/ci/security.sh; then
  printf 'security lane must not suppress Zizmor findings\n' >&2
  exit 1
fi
printf 'ci doctor ok: governed Jankurai and local lane dispatch\n'
