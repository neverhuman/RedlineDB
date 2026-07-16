set shell := ["bash", "-euo", "pipefail", "-c"]

default: fast

fast:
	bash ops/ci/fast.sh

test:
	cargo test --locked --workspace --all-targets

backend-oracles:
	cargo test --locked -p db-shim --all-targets --no-default-features --features oracle-sqlite
	cargo test --locked -p db-shim --all-targets --no-default-features --features oracle-postgres

security:
	bash ops/ci/security.sh

score:
	bash ops/ci/jankurai.sh

contract-drift:
	bash ops/ci/contract-drift.sh

artifact-support:
	bash ops/ci/artifact-support.sh

coverage:
	bash ops/ci/coverage.sh

ci-doctor:
	bash scripts/ci-doctor.sh

required:
	bash ops/ci/required.sh
