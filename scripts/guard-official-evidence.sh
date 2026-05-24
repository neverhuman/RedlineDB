#!/usr/bin/env bash
# Guard the official evidence boundary: RedlineDB may own wrappers and pinned
# config, but official report artifacts must come from redline-testing.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

fail=0

report_error() {
    printf 'official evidence guard: %s\n' "$*" >&2
    fail=1
}

if grep -nF 'cargo run -p redlinedb-bench --bin sqlite_parity' .jankurai/generated-zones.toml \
  || grep -nF 'cargo run -p redlinedb-bench --release --bin sqlite_parity' .jankurai/generated-zones.toml; then
    report_error "generated zones must not route official artifacts through in-tree sqlite_parity"
fi

if rg -n --glob '!scripts/guard-official-evidence.sh' \
  'redlinedb-bench .*--bin sqlite_parity -- (run|compare|report|sentinel|jankurai-compare)' \
  scripts ops just .github .jankurai README.md docs/testing.md docs/sqlite-parity.md crates/bench/src \
  >/dev/null; then
    rg -n --glob '!scripts/guard-official-evidence.sh' \
      'redlinedb-bench .*--bin sqlite_parity -- (run|compare|report|sentinel|jankurai-compare)' \
      scripts ops just .github .jankurai README.md docs/testing.md docs/sqlite-parity.md crates/bench/src >&2
    report_error "legacy in-tree sqlite_parity producer command is forbidden; use the pinned redline-testing binary"
fi

if rg -n --glob '!scripts/guard-official-evidence.sh' -- '--local-diagnostics' \
  scripts ops just .github .jankurai README.md docs/testing.md docs/sqlite-parity.md crates/bench/src \
  >/dev/null; then
    rg -n --glob '!scripts/guard-official-evidence.sh' -- '--local-diagnostics' \
      scripts ops just .github .jankurai README.md docs/testing.md docs/sqlite-parity.md crates/bench/src >&2
    report_error "unbound local diagnostics mode is forbidden for SQLite parity evidence"
fi

for legacy_lane in sqlite-parity-scale-smoke sqlite-parity-scale-ci sqlite-parity-volatile-sentinel sqlite-parity-scale-full; do
    if grep -nF "$legacy_lane:" just/lanes.just >/dev/null; then
        report_error "legacy SQLite parity lane must not be exposed in just/lanes.just: $legacy_lane"
    fi
    if grep -nF "name = \"$legacy_lane\"" .jankurai/proof-lanes.toml >/dev/null; then
        report_error "legacy SQLite parity lane must not be exposed in proof-lanes: $legacy_lane"
    fi
done

if ! grep -n 'reject_legacy_sqlite_parity_lane' scripts/just/run.sh >/dev/null; then
    report_error "legacy SQLite parity script lanes must hard-fail"
fi

if ! grep -n 'reject_legacy_producer' crates/bench/src/sqlite_parity/cli.rs >/dev/null; then
    report_error "redlinedb-bench sqlite_parity producer subcommands must hard-fail"
fi

if grep -nF 'beyond-postgres-reference' .github/workflows/*.yml .github/workflows/*.yaml 2>/dev/null; then
    report_error "GitHub CI must not run the in-tree beyond-postgres-reference lane as official evidence"
fi

for lane in redline-testing-official sqlite-parity-report-update sqlite-parity-report-check; do
    if ! grep -n "$lane" scripts/just/run.sh >/dev/null; then
        report_error "missing official wrapper lane in scripts/just/run.sh: $lane"
    fi
done

if ! grep -n 'redline_testing_bin="$(ci_install_redline_testing)"' scripts/just/run.sh >/dev/null; then
    report_error "official wrapper script must install pinned redline-testing"
fi

if ! grep -n 'ci_assert_redline_testing_official_artifacts' scripts/just/run.sh >/dev/null; then
    report_error "official wrapper script must assert the redline-testing evidence bundle"
fi

if ! grep -n 'process-redline-testing-evidence.sh target/redline-testing' scripts/just/run.sh >/dev/null; then
    report_error "official wrapper script must process redline-testing evidence"
fi

if ! grep -n 'official-evidence.processed.json' scripts/just/run.sh scripts/process-redline-testing-evidence.sh >/dev/null; then
    report_error "official wrapper script must emit the processed evidence artifact"
fi

if ! grep -n -- '--official-evidence "$official_evidence"' scripts/just/run.sh >/dev/null; then
    report_error "SQLite parity report args must bind report input to processed official evidence"
fi

if ! grep -n 'stage_sqlite_report_official_evidence' scripts/just/run.sh >/dev/null; then
    report_error "SQLite parity report update must stage processed official evidence into the report bundle"
fi

if ! grep -n 'report "${sqlite_parity_report_args_result' scripts/just/run.sh >/dev/null; then
    report_error "SQLite parity report lane must call redline-testing report"
fi

if ! grep -nF 'bash ops/ci/sqlite-parity-report.sh publish-pr' .github/workflows/sqlite-parity-report.yml >/dev/null; then
    report_error "SQLite parity report workflow must publish through the external runner wrapper"
fi

if ! grep -nF 'redline-testing-official' .github/workflows/ci.yml >/dev/null; then
    report_error "CI must route the official parity gate through redline-testing-official"
fi

if ! grep -nF 'official-evidence-guard' .github/workflows/ci.yml >/dev/null; then
    report_error "CI must keep the official evidence guard job wired into the PR gate"
fi

if ! grep -n 'actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a' .github/workflows/ci.yml >/dev/null; then
    report_error "CI must upload the pinned redline-testing evidence artifact bundle"
fi

if ! grep -n 'target/redline-testing/\*\*' .github/workflows/ci.yml >/dev/null; then
    report_error "CI must upload the full redline-testing evidence bundle"
fi

if ! grep -n 'CI_REDLINE_TESTING_EXPECTED_TARBALL_SHA256' ops/ci/lib.sh >/dev/null; then
    report_error "redline-testing tarball SHA256 must be hard-pinned in ops/ci/lib.sh"
fi

if ! grep -n 'gh attestation verify' ops/ci/lib.sh >/dev/null; then
    report_error "redline-testing release attestation verification is missing"
fi

exit "$fail"
