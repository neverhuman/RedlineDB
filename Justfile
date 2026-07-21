set shell := ["bash", "-euo", "pipefail", "-c"]

redlinectl := invocation_directory() / "redlinectl"

default: check

docker-parity *args:
  {{redlinectl}} docker-parity {{args}}

fast:
  cargo check --locked
  cargo test --locked

check:
  cargo fmt --check
  cargo clippy --locked --all-targets -- -D warnings
  cargo test --locked
  {{redlinectl}} review-lock-verify

required:
  bash scripts/ci-local.sh required

security:
  mkdir -p target/security
  CARGO_NET_OFFLINE=true cargo audit --no-fetch --deny warnings
  @printf '%s\n' 'jankurai-security-step={"label":"cargo-audit","tool":"cargo-audit","shell_command":"CARGO_NET_OFFLINE=true cargo audit --no-fetch --deny warnings","status":"ran","advisory":false,"exit_code":0}'
  cargo deny --frozen --all-features check
  @printf '%s\n' 'jankurai-security-step={"label":"cargo-deny","tool":"cargo-deny","shell_command":"cargo deny --frozen --all-features check","status":"ran","advisory":false,"exit_code":0}'
  gitleaks detect --source . --config .gitleaks.toml --no-banner --redact --no-git
  @printf '%s\n' 'jankurai-security-step={"label":"gitleaks","tool":"gitleaks","shell_command":"gitleaks detect","status":"ran","advisory":false,"exit_code":0}'
  actionlint .github/workflows/*.yml
  @printf '%s\n' 'jankurai-security-step={"label":"actionlint","tool":"actionlint","shell_command":"actionlint .github/workflows/*.yml","status":"ran","advisory":false,"exit_code":0}'
  zizmor --offline --min-severity high .github/workflows
  @printf '%s\n' 'jankurai-security-step={"label":"zizmor","tool":"zizmor","shell_command":"zizmor --offline --min-severity high .github/workflows","status":"ran","advisory":false,"exit_code":0}'
  SYFT_CHECK_FOR_APP_UPDATE=false syft dir:. -o spdx-json=target/security/redline-split-ops.spdx.json
  @printf '%s\n' 'jankurai-security-step={"label":"syft","tool":"syft","shell_command":"SYFT_CHECK_FOR_APP_UPDATE=false syft dir:.","status":"ran","advisory":false,"exit_code":0}'
  ./redlinectl security-receipt target/security/evidence.json

score:
  #!/usr/bin/env bash
  set -euo pipefail
  source ops/ci/lib.sh
  require_jankurai
  test -z "$(git status --porcelain=v1 --untracked-files=all)"
  mkdir -p target/jankurai/coverage
  install -m 0644 agent/jankurai-baseline.json target/jankurai/accepted-baseline.json
  "$JANKURAI_BIN" coverage audit . --config agent/coverage-sources.toml --json target/jankurai/coverage/coverage-audit.json --md target/jankurai/coverage/coverage-audit.md
  "$JANKURAI_BIN" audit . --full --mode ratchet --baseline target/jankurai/accepted-baseline.json --policy agent/audit-policy.toml --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md --repair-queue-jsonl target/jankurai/repair-queue.jsonl --no-score-history
  ./redlinectl audit-verify target/jankurai/repo-score.json
  test -z "$(git status --porcelain=v1 --untracked-files=all)"

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
