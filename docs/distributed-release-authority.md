# Distributed release authority

`9.0.0-distributed.1` uses a release contract separate from the historical
appliance verifier. The active `8.0.1` manifest remains unchanged until its own
protected authority lifecycle is complete.

The candidate and supporting proof documents have closed schemas:

- `schemas/distributed-release.v1.schema.json`
- `schemas/soak-status.v1.schema.json`
- `schemas/accelerated-qualification.v1.schema.json`
- `schemas/caddy-unchanged.v1.schema.json`
- `schemas/rollback-proof.v1.schema.json`
- `schemas/owner-signature.v1.schema.json`
- `schemas/locked-cargo-graph.v1.schema.json`

Validate their exact file bytes together:

```bash
splitctl distributed-validate \
  --release-spec /absolute/release.json \
  --soak-status /absolute/soak.json \
  --qualification /absolute/qualification.json \
  --release-dag /absolute/release-dag.json \
  --caddy-routes-before /absolute/caddy-routes-before.json \
  --caddy-routes-after /absolute/caddy-routes-after.json \
  --caddy-proof /absolute/caddy-unchanged.json \
  --redline-family-ci /absolute/redline-family-ci.json \
  --redline-lock /absolute/redline.lock.toml \
  --redline-lock-mirror /absolute/redline-lock-mirror.toml \
  --redline-jain-consumer /absolute/jain-consumer.json \
  --redline-jeryu-consumer /absolute/jeryu-consumer.json \
  --rollback-proof /absolute/rollback-proof.json \
  --signature-receipt /absolute/owner-one.json \
  --signature-receipt /absolute/owner-two.json
```

Every source tag must be exactly
`<repository>-v9.0.0-distributed.1-split.<positive revision>`, and the source
repository set must equal the exact release-DAG repository set. The artifact
inventory must contain Caddy, Hub, Node, worker, pack, Compose, installer, and
appliance-bundle classes. Every host's `image_set_sha256` is derived from the
four exact registry-readback image rows; hosts cannot assert unrelated image
sets.

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

`canonical_payload_sha256` is SHA-256 over the ASCII domain
`jain.distributed-release/v1`, one NUL byte, and compact canonical JSON after
removing both `canonical_payload_sha256` and `signatures`. This makes signing
non-circular. Each of exactly two distinct owner/key rows must bind an exact
`jain.owner-signature/v1` verifier receipt for that payload.

The validator opens every input through physical no-follow path components,
reads and validates metadata from the retained descriptor, then reopens the
same path identity. It rejects symlink components, hard-linked inputs, path
replacement, or in-place metadata change. Caddy validation hashes the exact
before/after route-array bytes and requires them byte-identical. Redline
validation consumes exact family-CI bytes, both byte-identical lock copies, and
both consumer evidence files; the engine tag/commit, proof-lock ID, family
receipt hash, consumer hashes, and cutover eligibility must agree throughout.
Rollback validation consumes the exact proof document and binds both the 7.0.6
source/artifact identity and identical distributed roll-forward artifact set.

Generate a deterministic, cycle-rejecting dependency DAG after the distributed
rows are onboarded into a reviewed manifest:

```bash
splitctl release-dag --manifest /absolute/repos.manifest.toml \
  --locked-cargo-graph /absolute/locked-cargo-graph.json \
  --release 9.0.0-distributed.1
```

The locked graph lists every manifest repository and each repository's exact
`Cargo.lock` digest (or `null` only when the manifest declares no Cargo
members), plus every cross-repository edge. The DAG rejects repository or edge
omission, unknown/duplicate edges, Cargo repositories without a lock digest,
cycles, waves outside `rollout_wave_order`, and dependencies assigned after
their consumers. Each DAG row carries the manifest's exact release tag,
commit, tree, and checksum, and validation rejects any source-matrix identity
that differs from those authority-derived values.

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
`--json PATH --apply` resolves and retains a physical no-follow output-parent
descriptor before indexing, rejects aliases or any location inside the indexed
root, and atomically creates a new file through that descriptor. It refuses to
replace existing evidence. Keeping the index outside the root avoids a
self-referential inventory that cannot bind its own final bytes.

These contracts do not authorize artifact publication, deployment, host
mutation, proof refresh, signing, promotion, GA, routing, or activation.
