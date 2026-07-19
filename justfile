set shell := ["bash", "-euo", "pipefail", "-c"]

default: check

fast: check

check:
  bash scripts/ci-local.sh required

pr-ci:
  bash scripts/ci-local.sh pr-ci

test:
  rtk cargo test --manifest-path tools/evidence-processor/Cargo.toml --locked
  bash ops/ci/github-mirror-contract.sh
  bash ops/ci/governed-jankurai-test.sh

score:
  bash scripts/just/run.sh score

security:
  bash tools/security-lane.sh

release: check security score

security-sbom:
  mkdir -p .jankurai/security
  syft . -o cyclonedx-json=.jankurai/security/sbom-syft.json

security-workflows:
  actionlint .github/workflows/*.yml

language-bad-behavior:
  bash ops/ci/governed-jankurai-test.sh

validate:
  ./scripts/guard-no-duplicate-engine.sh

doctor:
  REDLINE_SPLIT_ROOT="{{invocation_directory()}}/.." ../redline-split-ops/redlinectl doctor

family-validate:
  REDLINE_SPLIT_ROOT="{{invocation_directory()}}/.." ../redline-split-ops/redlinectl validate
