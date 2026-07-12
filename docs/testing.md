# Testing

Use the local lanes:

```bash
just fast
just required
just score
```

`fast` checks shell syntax, Rust control-plane tests, and the exact Python
parity boundary. `required` adds shellcheck when available, materializer tests,
manifest parsing, local-Jeryu policy validation, and fails on any Python file or
interpreter invocation that is not bound to named Rust parity evidence. `score`
runs Jankurai with the committed baseline ratchet.

For family-level proof, run each changed member repo's `just required` and
`just score` from its own checkout, then post statuses through
`ops/ci/split-host-ci.sh`.

## Release-candidate launch gates

The 8.0.0 result remains a release candidate until a separately authorized
production promotion. A formal launch gate must fail closed unless its raw
receipts prove all of the following:

- security scans, SBOM, provenance, signatures, and image inspection;
- backup and restore readiness for persistent state;
- monitoring for the CLI, web, Redline persistence, and SmartCluster daemon,
  client, and worker health;
- rollback rehearsal to the known `7.0.6` target;
- rate limit and abuse controls for uploads, storage, public routes, and
  bounded cleanup.

The fleet release proof starts with `./release-candidate.sh` (or
`./release-candidate.sh --plan` for an execution-free inventory). Rust owns the
wave plan, receipt reuse, exact-commit CI orchestration, canonical tag requests,
and the final rollout gate; the shell file only fixes the safety environment and
executes `splitctl`.

The AtomicSoul candidate commands are dry-run only. The budget, quota, spend
cap, stop condition, and kill switch policy is explicit: they set
`ATOMICSOUL_PUSH=0`, cannot change Caddy or production aliases, and use the
following cost budget: zero paid production operations, zero external compute,
and zero image pushes. Any attempted external write stops the run. Missing evidence keeps
`formal_ga = false`; it must never be restated as a passed gate.

## Repair Receipts

Every failing lane should leave a repair receipt or raw artifact under
`target/jankurai/`, `.jankurai/`, `target/security/`, or
`target/artifact-support/`. The receipt should include:

- `purpose`: the control-plane invariant being proven.
- `reason`: why the lane failed or why the artifact was emitted.
- `repair_hint`: the narrowest next command or file to inspect.
- `docs_url`: the local document that explains the lane.
- `common fixes`: rerun `just fast`, then `just required`, then `just score`
  after changing split generator, manifest, or CI files.
- `rerun command`: the exact `just ...` command that reproduces the failure.
