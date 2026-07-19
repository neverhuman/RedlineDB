# Distributed release authority

`9.0.0-distributed.1` uses a release contract separate from the historical
appliance verifier. The active `8.0.1` manifest remains unchanged until its own
protected authority lifecycle is complete.

The three closed documents are:

- `schemas/distributed-release.v1.schema.json`
- `schemas/soak-status.v1.schema.json`
- `schemas/accelerated-qualification.v1.schema.json`

Validate their exact file bytes together:

```bash
splitctl distributed-validate \
  --release-spec /absolute/release.json \
  --soak-status /absolute/soak.json \
  --qualification /absolute/qualification.json
```

The release spec binds the SHA-256 of the exact soak and qualification files.
The no-soak receipt also binds the exact qualification file as its replacement
receipt. This candidate accepts only `status=not_run`, `duration_seconds=0`,
`reason=unrouted_preproduction`, `formal_ga=false`, `public_routed=false`, and
`activation_eligible=false`. Accelerated qualification is never described as a
passed soak.

`source_freeze_sha256` is SHA-256 over source rows sorted by repository, each
encoded as:

```text
repository NUL tag NUL commit NUL tree NUL checksum_sha256 LF
```

`artifact_set_sha256` uses artifact rows sorted by kind and name:

```text
kind NUL name NUL sha256 NUL readback_sha256 NUL signature_receipt_sha256 LF
```

The qualification seed is lowercase SHA-256 of the ASCII domain
`jain.accelerated-qualification/v1`, one NUL byte, and the lowercase
`source_freeze_sha256` text. Registry/readback artifact digests must be equal.
AtomicSoul, xbabe2, and xbabe3 must share one active epoch and retain the fixed
unrouted roles, quotas, and listeners. The external Caddy route digest and ETag
must be identical before and after staging.

Signature fields bind the digests of externally verified signature files and
require distinct owner/key identities. `distributed-validate` is not a
cryptographic signature verifier; candidate closeout must run the approved
signature verifier first and bind its receipts.

Generate a deterministic, cycle-rejecting dependency DAG after the distributed
rows are onboarded into a reviewed manifest:

```bash
splitctl release-dag --manifest /absolute/repos.manifest.toml \
  --release 9.0.0-distributed.1
```

Before adding evidence, check the permanent root against every historical root:

```bash
splitctl distributed-evidence-index \
  --release 9.0.0-distributed.1 \
  --root docs/release-evidence/9.0.0-distributed.1 \
  --historical-root-if-present docs/release-evidence/9.0.0-alpha.6 \
  --historical-root-if-present docs/release-evidence/9.0.0-appliance.1
```

The evidence walk is recursive and fail-closed. It rejects symlinks,
non-regular nodes, hard links, repeated current or historical content hashes,
repeated current or historical `receipt_id` values, and a historical root that
appears after being strictly asserted absent. An if-present root is compared
when it exists and recorded absent when a clean clone has no historical bytes.
`--json PATH --apply` creates a new output file outside the indexed root and
refuses to replace existing evidence. Keeping the index outside the root avoids
a self-referential inventory that cannot bind its own final bytes.

These contracts do not authorize artifact publication, deployment, host
mutation, proof refresh, signing, promotion, GA, routing, or activation.
