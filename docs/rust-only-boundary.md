# Rust-only boundary

The Jain split uses Rust for product code, deployment tooling, CI orchestration,
release metadata, and control-plane utilities. Python is permitted only for
explicit parity/oracle tests and their fixtures.

The sole exception authority is `python-parity-exceptions.toml`. Every entry names one exact file,
a justification, a Rust owner and evidence path, an output path, input and output SHA-256 values,
and either byte-identical or explicitly normalized semantic comparison. Directory globs do not
grant permission. The current registry covers frozen model-zoo oracle/parity inputs, the frozen
deploy export fixture, and two Rust-owned inline parity tests.

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
machine-readable receipt containing every verified exception, its registry hash,
every declared invocation, and every violation. A stale file, owner, evidence,
output, or hash is a hard failure. Adding a new exception requires evidence that
the Python file is a frozen parity oracle or fixture for a named Rust
implementation.

E2E promotion rule: do not expand mocked or live E2E coverage until every
repository passes the fast and required lanes, security/security-network lanes,
source coverage, and the Rust-only boundary check. E2E output is evidence, not a
substitute for healthy per-repository CI.
