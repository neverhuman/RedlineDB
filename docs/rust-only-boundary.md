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
- `jain-redline/redline-split-ops/scripts/redline_proof.py` and its focused test module;
  this belongs to the independent nested Redline control plane, is not Jain
  product/deploy code, and is bounded to receipt verification and lock derivation

`python-parity-exceptions.toml` is the machine authority for this inventory.
`splitctl python-boundary` verifies the exact path set and SHA-256 of every Python
file, the named Rust evidence and recorded outputs for parity entries, and the
declared invocation paths. A directory-prefix match is never sufficient.

The current `jain-python` customer SDK is recorded as
`temporary-retirement-debt`, not as final-v9 authorization. Its exact files are
hash-bound only so changes cannot pass unnoticed while its contract, CLI,
orchestration, reporting, and deployment behavior is retired or ported to Rust.
The C08 exporter is separately classified as an isolated offline training
generator and has no default CI, release, deploy, or runtime invocation path.

`splitctl` in `jain-split-ops` is the native replacement for CI-contract
refresh, authored-repo refresh, local-Jeryu validation, and bare-mirror refresh.
The portal and deploy control planes use native Rust commands for source
coverage, fleet execution, version consistency, deployment staging, lock
validation, and release receipts. New CI, deployment, release, or control-plane
work must not add Python. The nested Redline exception above remains owned and
tested in `jain-redline/redline-split-ops`; other exceptions must be a customer example or
an explicit parity/oracle test with a declared owner and exit criteria.

E2E promotion rule: do not expand mocked or live E2E coverage until every
repository passes the fast and required lanes, security/security-network lanes,
source coverage, and the Rust-only boundary check. E2E output is evidence, not a
substitute for healthy per-repository CI.
