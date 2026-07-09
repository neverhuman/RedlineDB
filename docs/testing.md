# Testing

Use the local lanes:

```bash
just fast
just required
just score
```

`fast` checks shell syntax and Python compilation. `required` adds shellcheck
when available, materializer tests, manifest parsing, and local-Jeryu policy
validation. `score` runs Jankurai with the committed baseline ratchet.

For family-level proof, run each changed member repo's `just required` and
`just score` from its own checkout, then post statuses through
`ops/ci/split-host-ci.sh`.

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
