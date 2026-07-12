# Rust-only boundary

The Jain split uses Rust for product code, deployment tooling, CI orchestration,
release metadata, and control-plane utilities. Python is permitted only for
explicit parity/oracle tests and their fixtures.

Current allowed Python surfaces:

- `jain-model-zoo/ops/parity/**`
- `jain-model-zoo/reference/**/parity/**`
- `jain-model-zoo/reference/**/oracle/**`
- `jain-deploy/ops/ci/testdata/**` when the file is test input/model data
- `jain-python/python/ai-service/**` for the customer-facing Python SDK,
  SageMaker/customer examples, and Python parity tests only
- `redline-split-ops/scripts/redline_proof.py` and its focused test module;
  this belongs to the independent nested Redline control plane, is not Jain
  product/deploy code, and is bounded to receipt verification and lock derivation

`splitctl` in `jain-split-ops` is the native replacement for CI-contract
refresh, authored-repo refresh, local-Jeryu validation, and bare-mirror refresh.
The portal and deploy control planes use native Rust commands for source
coverage, fleet execution, version consistency, deployment staging, lock
validation, and release receipts. New CI, deployment, release, or control-plane
work must not add Python. The nested Redline exception above remains owned and
tested in `redline-split-ops`; other exceptions must be a customer example or
an explicit parity/oracle test with a declared owner and exit criteria.

E2E promotion rule: do not expand mocked or live E2E coverage until every
repository passes the fast and required lanes, security/security-network lanes,
source coverage, and the Rust-only boundary check. E2E output is evidence, not a
substitute for healthy per-repository CI.
