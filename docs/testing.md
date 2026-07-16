# Testing and proof lanes

`bash scripts/ci-local.sh required` is the one composed local contract. It runs:

- Rust formatting, locked checks/tests, clippy, and SQLite parity;
- secret/dependency scans plus an SPDX SBOM;
- the governed Jankurai 1.6.11 audit;
- native release and Docker contract checks;
- release artifact hashing; and
- Rust source coverage.

Protocol integration tests run a loopback-only deterministic server to validate
the magic/version handshake and invalid-magic rejection. Adapter integration
tests use in-memory SQLite to prove namespace expansion and transaction rollback.
The live Redline service smoke binary is explicit and separately scheduled; no
required test reaches a public or shared database.

Generated proof output lives under `target/` except the reviewed Jankurai score
copy under `agent/`. A passing local lane is not a merge receipt: the protected
workflow must rerun the required context at the exact pull-request head.

## Agent-readable failures

- Purpose: keep protocol and adapter failures classifiable without generic
  success/failure strings.
- Reason: operators must distinguish transport, server, configuration, and
  not-found outcomes before attempting repair.
- Common fixes: validate the configured backend and address, rerun the
  loopback protocol integration test, and inspect the exact server error.
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
