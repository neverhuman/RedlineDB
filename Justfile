set shell := ["bash", "-euo", "pipefail", "-c"]

redlinectl := "{{invocation_directory()}}/redlinectl"

default: check

fast:
  cargo check --locked
  cargo test --locked

check:
  cargo fmt --check
  cargo clippy --locked --all-targets -- -D warnings
  cargo test --locked
  {{redlinectl}} validate

required:
  bash scripts/ci-local.sh required

security:
  bash scripts/security.sh

score:
  bash scripts/score.sh

release-readiness:
  bash ops/ci/release-readiness.sh

doctor:
  bash scripts/ci-doctor.sh

validate:
  {{redlinectl}} validate

lock-verify:
  {{redlinectl}} lock-verify

test:
  cargo test --locked

clone-dry-run:
  {{redlinectl}} clone --dry-run

update:
  {{redlinectl}} update

family-ci receipt="target/release-evidence/redline-family-ci.json":
  {{redlinectl}} family-ci --receipt "{{receipt}}"

proof-refresh family_ci jain_evidence jeryu_evidence receipt="target/release-evidence/redline-proof-refresh.json":
  {{redlinectl}} proof-refresh --family-ci "{{family_ci}}" --jain-evidence "{{jain_evidence}}" --jeryu-evidence "{{jeryu_evidence}}" --receipt "{{receipt}}"

cutover-verify:
  {{redlinectl}} cutover-verify
