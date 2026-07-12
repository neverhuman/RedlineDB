# Rust-only boundary

The Jain split uses Rust for product code, deployment tooling, CI orchestration,
release metadata, and control-plane utilities. Python is permitted only for
explicit parity/oracle tests and their fixtures.

Current allowed Python surfaces:

- `jain-model-zoo/ops/parity/**`
- `jain-model-zoo/reference/**/parity/**`
- `jain-model-zoo/reference/**/oracle/**`
- `jain-deploy/ops/ci/testdata/invention-export/model.py`, which is test input
  consumed by the Rust export compatibility suite

There are no product, SDK, control-plane, CI, release, benchmark, paper, or
deployment exceptions. In particular, `jain-python/python/ai-service/**` is not
eligible for the v8 release graph in its current form, and the legacy Redline
benchmark/paper Python scripts are blockers until ported to Rust or removed.

`splitctl` in `jain-split-ops` is the native replacement for CI-contract
refresh, authored-repo refresh, local-Jeryu validation, and bare-mirror refresh.
The portal and deploy control planes use native Rust commands for source
coverage, fleet execution, version consistency, deployment staging, lock
validation, and release receipts. New CI, deployment, release, or control-plane
work must not add Python. `splitctl python-boundary` fails closed and emits a
machine-readable receipt containing every allowed parity file and every
violation. Adding a new exception requires evidence that the Python file is a
parity oracle or fixture for a named Rust implementation; directory names alone
do not create an exception.

E2E promotion rule: do not expand mocked or live E2E coverage until every
repository passes the fast and required lanes, security/security-network lanes,
source coverage, and the Rust-only boundary check. E2E output is evidence, not a
substitute for healthy per-repository CI.
