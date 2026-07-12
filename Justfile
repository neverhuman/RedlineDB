set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

required:
  ./ops/ci/required.sh

jeryu-doctor:
  cargo run --locked --quiet -- jeryu-doctor --manifest repos.manifest.toml

jeryu-ready:
  cargo run --locked --quiet -- jeryu-doctor --manifest repos.manifest.toml

jeryu-ready-apply:
  cargo run --locked --quiet -- jeryu-doctor --manifest repos.manifest.toml --fix-remotes --register-family

jeryu-repos:
  cargo run --locked --quiet -- jeryu-local repo-list

jeryu-prs repo:
  cargo run --locked --quiet -- jeryu-local pr-list --repo "{{repo}}"

managed-repos:
  cargo run --locked --quiet -- managed-repos --manifest repos.manifest.toml --json

sync-derived:
  cargo run --locked --quiet -- sync-derived-manifests --manifest repos.manifest.toml

sync-derived-apply:
  cargo run --locked --quiet -- sync-derived-manifests --manifest repos.manifest.toml --apply

bootstrap-main repo remote reviewed_commit:
  cargo run --locked --quiet -- bootstrap-main --repo "{{repo}}" --remote "{{remote}}" --reviewed-commit "{{reviewed_commit}}"

bootstrap-main-apply repo remote reviewed_commit:
  cargo run --locked --quiet -- bootstrap-main --repo "{{repo}}" --remote "{{remote}}" --reviewed-commit "{{reviewed_commit}}" --apply

immutable-tag repo remote tag commit:
  cargo run --locked --quiet -- immutable-tag --repo "{{repo}}" --remote "{{remote}}" --tag "{{tag}}" --commit "{{commit}}"

immutable-tag-apply repo remote tag commit:
  cargo run --locked --quiet -- immutable-tag --repo "{{repo}}" --remote "{{remote}}" --tag "{{tag}}" --commit "{{commit}}" --apply

jeryu-pr-ready repo number:
  cargo run --locked --quiet -- jeryu-local pr-ready --repo "{{repo}}" --number "{{number}}"

jeryu-pr-ready-apply repo number:
  cargo run --locked --quiet -- jeryu-local pr-ready --repo "{{repo}}" --number "{{number}}" --apply

jeryu-pr-close repo number:
  cargo run --locked --quiet -- jeryu-local pr-close --repo "{{repo}}" --number "{{number}}"

jeryu-pr-close-apply repo number:
  cargo run --locked --quiet -- jeryu-local pr-close --repo "{{repo}}" --number "{{number}}" --apply

jeryu-pr-merge repo number:
  cargo run --locked --quiet -- jeryu-local pr-merge --repo "{{repo}}" --number "{{number}}"

jeryu-pr-merge-apply repo number:
  cargo run --locked --quiet -- jeryu-local pr-merge --repo "{{repo}}" --number "{{number}}" --apply

jeryu-pr-approve repo number expected_head body="Reviewed release-critical change.":
  cargo run --locked --quiet -- jeryu-local pr-approve --repo "{{repo}}" --number "{{number}}" --expected-head "{{expected_head}}" --body "{{body}}"

jeryu-pr-approve-apply repo number expected_head body="Reviewed release-critical change.":
  cargo run --locked --quiet -- jeryu-local pr-approve --repo "{{repo}}" --number "{{number}}" --expected-head "{{expected_head}}" --body "{{body}}" --apply

jeryu-protection repo required_check:
  cargo run --locked --quiet -- jeryu-local protection-apply --repo "{{repo}}" --required-check "{{required_check}}"

jeryu-protection-apply repo required_check:
  cargo run --locked --quiet -- jeryu-local protection-apply --repo "{{repo}}" --required-check "{{required_check}}" --apply

jeryu-protection-readback repo required_check:
  cargo run --locked --quiet -- jeryu-local protection-readback --repo "{{repo}}" --required-check "{{required_check}}"

verify-worktrees:
  cargo run --locked --quiet -- verify-worktrees --manifest repos.manifest.toml

fast:
  ./ops/ci/fast.sh

cargo-check:
  cargo check --locked -p jain-split-ops

check:
  ./ops/ci/check.sh

score:
  ./ops/ci/score.sh # jankurai audit . --mode ratchet --baseline target/jankurai/accepted-baseline.json --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md

security:
  ./ops/ci/security.sh # fail-closed gitleaks, cargo-audit, cargo-deny, Syft, and Grype

tool-adoption:
  ./ops/ci/tool-adoption.sh

contract-drift:
  ./ops/ci/contract-drift.sh

preflight:
  cargo run --locked --quiet -- preflight --manifest repos.manifest.toml --json target/preflight-report.json

artifact-support:
  ./ops/ci/artifact_support.sh

release-preflight:
  cargo run --locked --quiet -- release-preflight --manifest repos.manifest.toml --json target/release-preflight.json

release-snapshot:
  cargo run --locked --quiet -- release-snapshot --manifest repos.manifest.toml --json docs/release-evidence/8.0.0/release-snapshot.json --apply

release-ci:
  ./ops/ci/required.sh
  ./ops/ci/security.sh
  ./ops/ci/score.sh
  ./ops/ci/contract-drift.sh

# Candidate-only release gate. Production application is intentionally absent.
release:
  just release-ci

release-artifacts version="8.0.0":
  ATOMICSOUL_PUSH=0 JAIN_RELEASE_VERSION="{{version}}" cargo run --locked --manifest-path ../jain-deploy/Cargo.toml -p jain-deploy-engine --bin deployctl -- build-local --version "{{version}}" --gate-dir "target/release-evidence/{{version}}"

release-canary-dry-run version="8.0.0":
  ATOMICSOUL_PUSH=0 JAIN_RELEASE_VERSION="{{version}}" cargo run --locked --manifest-path ../jain-deploy/Cargo.toml -p jain-deploy-engine --bin deployctl -- canary-e2e --version "{{version}}" --slot jain-a --dry-run --gate-dir "target/release-evidence/{{version}}"

release-promote-dry-run digest version="8.0.0":
  ATOMICSOUL_PUSH=0 JAIN_RELEASE_VERSION="{{version}}" cargo run --locked --manifest-path ../jain-deploy/Cargo.toml -p jain-deploy-engine --bin deployctl -- promote-prod --version "{{version}}" --digest "{{digest}}" --dry-run --gate-dir "target/release-evidence/{{version}}"

release-rollback-dry-run to="7.0.6":
  ATOMICSOUL_PUSH=0 JAIN_RELEASE_VERSION=8.0.0 cargo run --locked --manifest-path ../jain-deploy/Cargo.toml -p jain-deploy-engine --bin deployctl -- rollback --to "{{to}}" --dry-run

release-status:
  cargo run --locked --quiet -- release-status --manifest repos.manifest.toml --json docs/release-evidence/8.0.0/release-status.json

refresh-authored:
  cargo run --locked --quiet -- refresh-ci-contract --authored

refresh-authored-repo repo:
  cargo run --locked --quiet -- refresh-ci-contract --authored --repo {{repo}}

refresh-ci:
  cargo run --locked --quiet -- refresh-ci-contract

refresh-ci-repo repo:
  cargo run --locked --quiet -- refresh-ci-contract --repo {{repo}}

refresh-mirrors:
  cargo run --locked --quiet -- refresh-bare-mirrors --manifest repos.manifest.toml

refresh-mirrors-apply:
  cargo run --locked --quiet -- refresh-bare-mirrors --manifest repos.manifest.toml --apply

atomicsoul-dry-run:
  cd ../jain-deploy && ATOMICSOUL_PUSH=0 JAIN_RELEASE_VERSION=8.0.0 ./scripts/atomicsoul-dry-run.sh

profile:
  printf '%s\n' "split-ops-control-plane"
