#!/usr/bin/env bash
# Release-readiness lane: assert the release control surface exists (version
# source, changelog, release + rollback docs, evidence paths) and emit a
# receipt.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"
ensure_artifacts

log "release-readiness: validating release evidence surface"
python3 - <<'PY'
import json
from pathlib import Path

required_files = [
    "CHANGELOG.md",
    "docs/release.md",
    "docs/testing.md",
    "docs/operations.md",
    "agent/cost-budget.toml",
    "apps/api/Cargo.toml",
]
missing = [p for p in required_files if not Path(p).exists()]
release = Path("docs/release.md").read_text() if Path("docs/release.md").exists() else ""
testing = Path("docs/testing.md").read_text() if Path("docs/testing.md").exists() else ""
required_terms = [
    "bash ops/ci/pr-ci.sh",
    "ops/ci/security.sh",
    "ops/ci/release-readiness.sh",
    "target/jankurai",
    "rollback",
    "version",
]
missing_terms = [t for t in required_terms if t not in release and t not in testing]
receipt = {
    "ok": not missing and not missing_terms,
    "version_source": "apps/api/Cargo.toml",
    "required_files": required_files,
    "missing_files": missing,
    "missing_terms": missing_terms,
    "artifact_paths": [
        "target/jankurai/release-readiness.json",
        "target/jankurai/cost-budget.json",
        "target/jankurai/security/evidence.json",
    ],
}
Path("target/jankurai/release-readiness.json").write_text(json.dumps(receipt, indent=2) + "\n")
if not receipt["ok"]:
    raise SystemExit(f"release readiness missing evidence: files={missing} terms={missing_terms}")
PY

log "release-readiness: complete"
