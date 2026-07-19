# Jain 9.0.0-appliance.1 production-hotfix operator

The emergency appliance is a distinct `production-hotfix` release with
`formal_ga=false`. It neither changes nor supplies evidence for
`9.0.0-alpha.6`.

The canonical authority is
`authority/jain-9.0.0-appliance.1.production-hotfix.toml`; the only evidence
root is `docs/release-evidence/9.0.0-appliance.1`. The authority locks the
public host, three-role roster, installer identity, rollback route contract,
six-hour stage soak, one-hour signed-cookie canary, and the complete set of
action-envelope-gated operations.

Run the exact interface from this repository:

```bash
splitctl appliance-release plan \
  --authority authority/jain-9.0.0-appliance.1.production-hotfix.toml \
  --evidence-root docs/release-evidence/9.0.0-appliance.1

splitctl appliance-release build \
  --spec docs/release-evidence/9.0.0-appliance.1/release-spec.json

splitctl appliance-release publish \
  --spec docs/release-evidence/9.0.0-appliance.1/release-spec.json \
  --action-envelope /protected/fd-bound/publish-envelope.json --apply

splitctl appliance-release stage --host atomicsoul \
  --spec docs/release-evidence/9.0.0-appliance.1/release-spec.json \
  --action-envelope /protected/fd-bound/stage-envelope.json --apply

splitctl appliance-release qualify \
  --spec docs/release-evidence/9.0.0-appliance.1/release-spec.json \
  --minimum-soak 6h

splitctl appliance-release canary --host www.neverhuman.org \
  --cohort signed-cookie --duration 1h \
  --spec docs/release-evidence/9.0.0-appliance.1/release-spec.json \
  --action-envelope /protected/fd-bound/canary-envelope.json --apply

splitctl appliance-release promote --host www.neverhuman.org \
  --expected-caddy-etag '"fresh-strong-etag"' \
  --spec docs/release-evidence/9.0.0-appliance.1/release-spec.json \
  --action-envelope /protected/fd-bound/promotion-envelope.json --apply

splitctl appliance-release rollback --host www.neverhuman.org \
  --to-receipt docs/release-evidence/9.0.0-appliance.1/production/pre-v9.json \
  --spec docs/release-evidence/9.0.0-appliance.1/release-spec.json \
  --action-envelope /protected/fd-bound/rollback-envelope.json --apply
```

Without `--apply`, an operation reads and validates local inputs, emits a
deterministic JSON plan, and changes no state. Apply additionally requires the
plan to report no blockers, a fresh two-owner cosign envelope bound to the
exact authority/spec/plan digests, an unconsumed nonce in the root-owned nonce
store, and a root-owned exact-digest Rust operator. The verified request is
sent to that operator on stdin with an empty environment. No shell command,
ambient credential, secret argv, verification bypass, automatic CAS retry, or
mutable tag is accepted.

The checked-in spec is intentionally pending. `null` means an identity has not
yet been proven; it is never a zero pin. Protected source, artifact, rollback,
and qualification lanes update the spec only with their exact receipts. A
failed or ambiguous operator attempt consumes its nonce and emits a failure
receipt, so a new signed envelope is required after readback.
