# Testing and proof lanes

`bash scripts/ci-local.sh required` is the one composed local contract. It runs:

- Rust formatting, locked checks/tests, clippy, exact isolated dependency-tree guards for all three
  features, and the genuine in-memory SQLite oracle;
- secret/dependency scans plus an SPDX SBOM;
- the governed Jankurai 1.6.11 audit;
- native release and Docker contract checks;
- release artifact hashing; and
- Rust source coverage.

Protocol integration tests run a loopback-only deterministic server to validate the Redline
magic/version handshake and invalid-magic rejection. The dependency guard separately proves the
default Redline, explicit Redline, SQLite, and Postgres Cargo closures. The
`db-shim.used-operations/v2` corpus is table-driven and backend-neutral; it asserts identical
integer, real, text, and blob null round trips plus success-only execution. Ordinary required runs
it against real in-memory SQLite. `JAIN_RELEASE_CI=1` changes build custody but does not request
network credentials. `bash scripts/ci-local.sh family-release` is the separate armed family lane:
it requires `REDLINE_CORPUS_DSN` and `POSTGRES_CORPUS_DSN` and runs the identical corpus against
both real services. Missing or substitute DSNs fail closed. No result is described as arbitrary or
full SQL parity.

Security evidence distinguishes local readiness from governed host proof. Local runs bind the
actual clean advisory checkout they inspect. Release host CI must provide both
`JAIN_PINNED_ADVISORY_DB` and `JAIN_PINNED_ADVISORY_COMMIT`; the lane validates that exact clean
commit, derives its full tree identity, and records both commit and tree in the receipt.

Generated proof output lives under `target/` except the reviewed Jankurai score
copy under `agent/`. A passing local lane is not a merge receipt: the protected
workflow must rerun the required context at the exact pull-request head.
Normal and required score runs never rewrite tracked source. An author refreshes the reviewed
snapshot explicitly with `bash scripts/ci-local.sh score-update`, then commits it before exact-head
proof.

## Agent-readable failures

- Purpose: keep protocol and adapter failures classifiable without generic
  success/failure strings.
- Reason: operators must distinguish transport, server, configuration, and
  not-found outcomes before attempting repair.
- Common fixes: validate the selected Cargo feature and DSN, rerun the loopback protocol integration
  test, and inspect the stable error category plus exact server error.
- `repair_hint`: start with the narrow crate integration test, then run the
  composed required lane.
- `docs_url`: `docs/testing.md#agent-readable-failures`.

## Launch gate boundaries

This candidate is not launch-eligible by local test output alone. Security and
SBOM artifacts must bind the merged commit. Backup/restore evidence must prove
the central database can recover without rewriting native Redline identity.
Monitoring must cover availability, protocol failures, and storage pressure.
Rollback must select a previous immutable tag and a verified data backup.
Network exposure, authentication, tenancy, rate limits, and other abuse controls
remain mandatory deployment evidence; their absence blocks launch rather than
being waived by this repository's host-local tests.
