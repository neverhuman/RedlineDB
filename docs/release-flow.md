# Durable release status

`splitctl release-flow` is the protected, read-only release status reducer for
Jain 8.0.1. It validates the candidate metadata, authority and derived
manifests, physical primary checkouts, immutable repository identities,
Redline proof evidence, protected-forge readback, and an optional Cloud release
spec. It never tags, approves, merges, stages, promotes, rolls back, changes a
route, or launches CI.

The tracked deployment wrapper is `ops/release-flow-deploy.sh`. The installed
root wrapper must be byte-identical to that protected file and dispatches only
to `/usr/local/libexec/jain/splitctl` with the fixed manifest and evidence root.

## Status and resume

The default command writes nothing:

```text
splitctl release-flow \
  --manifest /home/ubuntu/jain-split/jain-split-ops/repos.manifest.toml \
  --evidence-root /home/ubuntu/jain-split/jain-split-ops/docs/release-evidence/8.0.1
```

Add `--token-file <absolute-readback-token>` only when protected-forge checks,
commit status, protection, immutable refs, and approvals are ready for
readback. Credential paths, fingerprints, and bytes are never included in the
report. Add `--cloud-spec <absolute-spec>` to include the Cloud contract gate.

`--record <new-path>` exclusively creates one mode-0600 receipt beneath the
declared evidence root. The destination and every parent must already be a
physical path, and an existing, linked, replaced, or out-of-root destination is
refused. The receipt carries an integrity SHA-256 and is fsynced with its
parent. No other path is changed.

`--resume <prior-receipt>` recomputes mutable gates only after verifying the
prior schema and integrity digest, manifest hash, control-plane head/tree/status
identity, predecessor binding, and every evidence reference. Any drift or
tampering is a hard refusal.

The JSON result contains one deterministic `next_action` argv array and a
required role. `overall_status=ready` is status evidence only; explicit Cloud
phase application remains the separate protected `deployctl cloud stage`,
`qualify`, `promote`, or `rollback` interface and still requires its own
authorization and action envelope.
