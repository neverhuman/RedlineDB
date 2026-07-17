set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

required:
  ./ops/ci/required.sh

# Privileged pre-install namespace acceptance; never run inside the unprivileged CI worker.
jeryu-transport-acceptance:
  ./ops/ci/jeryu-branch-push-integration-test.sh

jeryu-doctor:
  cargo run --locked --quiet -- jeryu-doctor --manifest repos.manifest.toml

jeryu-ready:
  cargo run --locked --quiet -- jeryu-doctor --manifest repos.manifest.toml

jeryu-ready-apply:
  cargo run --locked --quiet -- jeryu-doctor --manifest repos.manifest.toml --fix-remotes --register-family --install-hooks

jeryu-repos token_file:
  cargo run --locked --quiet -- jeryu-local repo-list --token-file "{{token_file}}"

jeryu-prs repo token_file:
  cargo run --locked --quiet -- jeryu-local pr-list --repo "{{repo}}" --token-file "{{token_file}}"

jeryu-branch-push repo repo_path branch expected_head:
  cargo run --locked --quiet -- jeryu-local branch-push --repo "{{repo}}" --repo-path "{{repo_path}}" --branch "{{branch}}" --expected-head "{{expected_head}}"

jeryu-branch-push-apply repo repo_path branch expected_head token_file:
  cargo run --locked --quiet -- jeryu-local branch-push --repo "{{repo}}" --repo-path "{{repo_path}}" --branch "{{branch}}" --expected-head "{{expected_head}}" --token-file "{{token_file}}" --apply

jeryu-pr-open repo title head expected_head base="main":
  cargo run --locked --quiet -- jeryu-local pr-open --repo "{{repo}}" --title "{{title}}" --head "{{head}}" --expected-head "{{expected_head}}" --base "{{base}}"

jeryu-pr-open-apply repo title head expected_head token_file base="main":
  cargo run --locked --quiet -- jeryu-local pr-open --repo "{{repo}}" --title "{{title}}" --head "{{head}}" --expected-head "{{expected_head}}" --base "{{base}}" --token-file "{{token_file}}" --apply

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

jeryu-pr-ready-apply repo number token_file:
  cargo run --locked --quiet -- jeryu-local pr-ready --repo "{{repo}}" --number "{{number}}" --token-file "{{token_file}}" --apply

jeryu-pr-close repo number:
  cargo run --locked --quiet -- jeryu-local pr-close --repo "{{repo}}" --number "{{number}}"

jeryu-pr-close-apply repo number token_file:
  cargo run --locked --quiet -- jeryu-local pr-close --repo "{{repo}}" --number "{{number}}" --token-file "{{token_file}}" --apply

jeryu-pr-merge repo number expected_head:
  cargo run --locked --quiet -- jeryu-local pr-merge --repo "{{repo}}" --number "{{number}}" --expected-head "{{expected_head}}"

jeryu-pr-merge-apply repo number expected_head token_file:
  cargo run --locked --quiet -- jeryu-local pr-merge --repo "{{repo}}" --number "{{number}}" --expected-head "{{expected_head}}" --token-file "{{token_file}}" --apply

jeryu-pr-approve repo number expected_head body="Reviewed release-critical change.":
  cargo run --locked --quiet -- jeryu-local pr-approve --repo "{{repo}}" --number "{{number}}" --expected-head "{{expected_head}}" --body "{{body}}"

jeryu-pr-approve-apply repo number expected_head token_file body="Reviewed release-critical change.":
  cargo run --locked --quiet -- jeryu-local pr-approve --repo "{{repo}}" --number "{{number}}" --expected-head "{{expected_head}}" --body "{{body}}" --token-file "{{token_file}}" --apply

jeryu-protection repo required_check:
  cargo run --locked --quiet -- jeryu-local protection-apply --repo "{{repo}}" --required-check "{{required_check}}"

jeryu-protection-apply repo required_check token_file:
  cargo run --locked --quiet -- jeryu-local protection-apply --repo "{{repo}}" --required-check "{{required_check}}" --token-file "{{token_file}}" --apply

jeryu-protection-readback repo required_check token_file:
  cargo run --locked --quiet -- jeryu-local protection-readback --repo "{{repo}}" --required-check "{{required_check}}" --token-file "{{token_file}}"

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
  version="$(awk -F'\"' '/^release_version = / {print $2; exit}' repos.manifest.toml)"; cargo run --locked --quiet -- release-snapshot --manifest repos.manifest.toml --json "docs/release-evidence/${version}/release-snapshot.json" --apply

release-ci:
  ./ops/ci/required.sh
  ./ops/ci/security.sh
  ./ops/ci/score.sh
  ./ops/ci/contract-drift.sh

# Candidate-only release gate. Production application is intentionally absent.
release:
  just release-ci

release-artifacts version="8.0.1":
  ATOMICSOUL_PUSH=0 JAIN_RELEASE_VERSION="{{version}}" cargo run --locked --manifest-path ../jain-deploy/Cargo.toml -p jain-deploy-engine --bin deployctl -- build-local --version "{{version}}" --gate-dir "target/release-evidence/{{version}}"

release-canary-dry-run version="8.0.1":
  ATOMICSOUL_PUSH=0 JAIN_RELEASE_VERSION="{{version}}" cargo run --locked --manifest-path ../jain-deploy/Cargo.toml -p jain-deploy-engine --bin deployctl -- canary-e2e --version "{{version}}" --slot jain-a --dry-run --gate-dir "target/release-evidence/{{version}}"

release-promote-dry-run digest version="8.0.1":
  ATOMICSOUL_PUSH=0 JAIN_RELEASE_VERSION="{{version}}" cargo run --locked --manifest-path ../jain-deploy/Cargo.toml -p jain-deploy-engine --bin deployctl -- promote-prod --version "{{version}}" --digest "{{digest}}" --dry-run --gate-dir "target/release-evidence/{{version}}"

release-rollback-dry-run to="7.0.6":
  version="$(awk -F'\"' '/^release_version = / {print $2; exit}' repos.manifest.toml)"; ATOMICSOUL_PUSH=0 JAIN_RELEASE_VERSION="${version}" cargo run --locked --manifest-path ../jain-deploy/Cargo.toml -p jain-deploy-engine --bin deployctl -- rollback --to "{{to}}" --dry-run

release-status:
  version="$(awk -F'\"' '/^release_version = / {print $2; exit}' repos.manifest.toml)"; cargo run --locked --quiet -- release-status --manifest repos.manifest.toml --json "docs/release-evidence/${version}/release-status.json"

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
  version="$(awk -F'\"' '/^release_version = / {print $2; exit}' repos.manifest.toml)"; cd ../jain-deploy && ATOMICSOUL_PUSH=0 JAIN_RELEASE_VERSION="${version}" ./scripts/atomicsoul-v8-dry-run.sh

profile:
  printf '%s\n' "split-ops-control-plane"
