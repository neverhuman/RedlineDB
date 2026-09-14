set shell := ["bash", "-euo", "pipefail", "-c"]

default: check

fast: check

check:
  bash scripts/ci-local.sh required

score:
  jankurai audit . --full --mode advisory --json .jankurai/repo-score.json --md .jankurai/repo-score.md --policy agent/audit-policy.toml

security:
  bash tools/security-lane.sh

release: check security score

security-sbom:
  mkdir -p .jankurai/security
  syft . -o cyclonedx-json=.jankurai/security/sbom-syft.json

security-workflows:
  actionlint .github/workflows/*.yml

language-bad-behavior:
  #!/usr/bin/env bash
  set -euo pipefail
  source ops/ci/lib.sh
  scratch=.jankurai/jankurai-src
  trap 'rm -rf "$scratch"' EXIT
  rm -rf "$scratch"
  ci_verify_jankurai_source
  git clone --depth 1 --branch "$CI_JANKURAI_TAG" "$CI_JANKURAI_GIT" "$scratch"
  test "$(git -C "$scratch" rev-parse HEAD)" = "$CI_JANKURAI_REV"
  cd "$scratch"
  cargo test -p jankurai --test language_bad_behavior --no-fail-fast

validate:
  ./scripts/guard-no-duplicate-engine.sh

doctor:
  REDLINE_SPLIT_ROOT="{{invocation_directory()}}/.." ../redline-split-ops/redlinectl doctor

family-validate:
  REDLINE_SPLIT_ROOT="{{invocation_directory()}}/.." ../redline-split-ops/redlinectl validate

# Entry point for the protected redline/required check: the existing lane, unchanged.
required: check
